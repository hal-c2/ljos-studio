//! The tools a canvas gesture drops through: where objects are, placing them,
//! lining them up, copying them. Each is one undo step however many objects
//! it touches, so a multi-select drag undoes as the person made it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::schema::Answers;
use ljos_mcp_core::{CallCtx, Cap, ErrorCode, SurfaceError, ToolDef, ToolRegistry, denied, typed};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use studio_core::place::{self, Align, Drag, Moving, Rect, Wanted};
use studio_core::preview::Placed;
use studio_core::{Address, Node, NodePath, Op, Step};

use crate::tools::{EditOut, MUTATE};
use crate::{Studio, error, lock};

/// Siblings told to a canvas for snapping: past this, a parent's guides are
/// noise and reading them costs more than the frame they serve.
const MOST_SIBLINGS: usize = 64;
/// How far a duplicate lands from its original, so it is seen as a new object.
const DUPLICATE_OFFSET: f64 = 16.0;

/// What `node_arrange` `copy` holds until the next one: the object as written and
/// the ids in it, so a paste can rename what its target already has.
#[derive(Clone, Debug)]
pub(crate) struct Copied {
    text: String,
    ids: Vec<String>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Action {
    Geometry,
    Place,
    Align,
    Copy,
    Paste,
    Duplicate,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArrangeArgs {
    action: Action,
    doc: String,
    /// For all but `place` and `paste`.
    #[serde(default)]
    nodes: Vec<Address>,
    /// Needed when a node is named by path.
    generation: Option<u64>,
    /// For `place`.
    places: Option<Vec<PlaceOne>>,
    /// For `align`: left, hcenter, right, top, vcenter, bottom, distribute_h,
    /// distribute_v. One object lines up with its parent.
    align: Option<Align>,
    /// For `paste`, into any document. Ids it has are renamed.
    parent: Option<Address>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Default, Serialize, JsonSchema)]
struct ArrangeOut {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    edit: Option<EditOut>,
    /// The objects placed or put, in the order asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<String>,
    /// `geometry`: each object asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    boxes: Vec<NodeGeometry>,
    /// `geometry`: the first object's parent, in window coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<Rect>,
    /// `geometry`: the parent's other children, for guides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    siblings: Vec<Sibling>,
    /// `copy`: each object as written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    copied: Vec<String>,
}

struct GeometryArgs {
    doc: String,
    nodes: Vec<Address>,
    generation: Option<u64>,
}

struct GeometryOut {
    nodes: Vec<NodeGeometry>,
    parent: Option<Rect>,
    siblings: Vec<Sibling>,
}

#[derive(Serialize, JsonSchema)]
struct NodeGeometry {
    node: String,
    /// In the parent's coordinates; absent when the preview has no such
    /// object (a `Loader` not loaded, a state that hides it).
    #[serde(skip_serializing_if = "Option::is_none")]
    local: Option<Rect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<Rect>,
    drag: Drag,
}

#[derive(Serialize, JsonSchema)]
struct Sibling {
    node: String,
    window: Rect,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlaceOne {
    node: Address,
    /// In the parent's coordinates. What is left out stays.
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
}

struct ManyOut {
    edit: EditOut,
    nodes: Vec<String>,
}

struct AlignArgs {
    doc: String,
    nodes: Vec<Address>,
    align: Align,
    generation: Option<u64>,
}

struct NodesArgs {
    doc: String,
    nodes: Vec<Address>,
    generation: Option<u64>,
}

struct PasteArgs {
    doc: String,
    parent: Address,
}

fn refused(kind: &str, message: impl Into<String>) -> SurfaceError {
    SurfaceError::new(ErrorCode::NotActionable, message).with_details(json!({ "kind": kind }))
}

fn parent_path(path: &NodePath) -> Option<NodePath> {
    (!path.0.is_empty()).then(|| NodePath(path.0[..path.0.len() - 1].to_vec()))
}

fn short(ty: &str) -> &str {
    ty.rsplit('.').next().unwrap_or(ty)
}

/// The objects the preview of `doc` has of `nodes`, by handle; the first copy
/// of a delegate speaks for all, since a file has one declaration of them.
/// `None` when no preview of it is showing.
fn placed_of(
    studio: &mut Studio,
    doc: &str,
    nodes: &[(String, Node)],
) -> Result<Option<HashMap<String, Placed>>, SurfaceError> {
    let mut ids = Vec::new();
    let mut types = Vec::new();
    for (_, node) in nodes {
        match &node.id {
            Some(id) if !ids.contains(id) => ids.push(id.clone()),
            Some(_) => {}
            None => {
                let ty = short(&node.ty).to_owned();
                if !types.contains(&ty) {
                    types.push(ty);
                }
            }
        }
    }
    let Some(placed) = crate::preview_tools::placed(studio, doc, &ids, &types)? else {
        return Ok(None);
    };
    let mut by_handle = HashMap::new();
    for one in placed {
        let Some(declared) = &one.declared else {
            continue;
        };
        if declared.file != doc {
            continue;
        }
        let Ok((handle, _)) =
            studio
                .session
                .declared_at(doc, declared.line as usize, declared.column as usize)
        else {
            continue;
        };
        by_handle.entry(handle).or_insert(one);
    }
    Ok(Some(by_handle))
}

/// The object at `path`, with its handle.
fn at_path(tree: &[(String, Node)], path: &NodePath) -> Option<(String, Node)> {
    tree.iter().find(|(_, n)| n.path == *path).cloned()
}

/// The types of what places `node`: the file's, and the preview's too when it
/// is known, since a component of the project can be a layout underneath.
fn parent_types<'a>(parent: Option<&'a Node>, placed: Option<&'a Placed>) -> Vec<&'a str> {
    let mut types: Vec<&str> = parent.iter().map(|p| p.ty.as_str()).collect();
    if let Some(placed) = placed {
        types.extend(placed.types.iter().map(String::as_str));
    }
    types
}

/// A freely placed object's numbers as the file has them: a preview that has
/// not reloaded yet would have the next nudge start from where it used to be.
fn as_written(node: &Node, drag: &Drag, mut local: Rect) -> Rect {
    if drag.moving == Moving::Position {
        let written = |name: &str, live: f64| {
            node.bindings
                .iter()
                .find(|b| b.name == name)
                .and_then(|b| b.value.parse().ok())
                .unwrap_or(live)
        };
        local.x = written("x", local.x);
        local.y = written("y", local.y);
    }
    local
}

fn geometry(studio: &mut Studio, a: &GeometryArgs) -> Result<GeometryOut, SurfaceError> {
    let (tree, generation) = studio.session.tree(&a.doc).map_err(error)?;
    if let Some(given) = a.generation
        && given != generation
    {
        return Err(error(studio_core::Error::Stale {
            doc: a.doc.clone(),
            given,
            current: generation,
        }));
    }
    let mut asked = Vec::new();
    for address in &a.nodes {
        asked.push(studio.session.resolve(&a.doc, address).map_err(error)?);
    }
    let first_parent = asked
        .first()
        .and_then(|(_, n)| parent_path(&n.path))
        .and_then(|p| at_path(&tree, &p));
    let siblings: Vec<(String, Node)> = first_parent
        .iter()
        .flat_map(|(_, parent)| {
            tree.iter().filter(move |(_, n)| {
                parent_path(&n.path).as_ref() == Some(&parent.path)
                    && matches!(n.path.0.last(), Some(Step::Child(_)))
            })
        })
        .filter(|(handle, _)| !asked.iter().any(|(h, _)| h == handle))
        .take(MOST_SIBLINGS)
        .cloned()
        .collect();
    let parents: Vec<(String, Node)> = asked
        .iter()
        .filter_map(|(_, n)| parent_path(&n.path).and_then(|p| at_path(&tree, &p)))
        .collect();
    let wanted: Vec<(String, Node)> = asked
        .iter()
        .chain(&parents)
        .chain(&siblings)
        .cloned()
        .collect();
    let Some(placed) = placed_of(studio, &a.doc, &wanted)? else {
        return Err(refused(
            "no_preview",
            format!(
                "no preview of `{}` is open; `preview_open` starts one",
                a.doc
            ),
        ));
    };
    let nodes = asked
        .iter()
        .map(|(handle, node)| {
            let parent = parent_path(&node.path).and_then(|p| at_path(&tree, &p));
            let parent_placed = parent.as_ref().and_then(|(h, _)| placed.get(h));
            let drag = place::drag_of(
                node,
                &parent_types(parent.as_ref().map(|(_, n)| n), parent_placed),
            );
            let here = placed.get(handle);
            NodeGeometry {
                node: handle.clone(),
                local: here.map(|p| as_written(node, &drag, p.local)),
                window: here.map(|p| p.window),
                drag,
            }
        })
        .collect();
    Ok(GeometryOut {
        nodes,
        parent: first_parent
            .as_ref()
            .and_then(|(h, _)| placed.get(h))
            .map(|p| p.window),
        siblings: siblings
            .iter()
            .filter_map(|(handle, _)| {
                placed.get(handle).map(|p| Sibling {
                    node: handle.clone(),
                    window: p.window,
                })
            })
            .collect(),
    })
}

/// The edits placing each of `places` asks for, all or none.
fn place_ops(
    studio: &mut Studio,
    doc: &str,
    places: &[(Address, Wanted)],
) -> Result<(Vec<Op>, Vec<String>), SurfaceError> {
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    let mut targets = Vec::new();
    for (address, wanted) in places {
        let (handle, node) = studio.session.resolve(doc, address).map_err(error)?;
        targets.push((handle, node, *wanted));
    }
    let wanted: Vec<(String, Node)> = targets
        .iter()
        .flat_map(|(handle, node, _)| {
            let parent = parent_path(&node.path).and_then(|p| at_path(&tree, &p));
            std::iter::once((handle.clone(), node.clone())).chain(parent)
        })
        .collect();
    let placed = placed_of(studio, doc, &wanted)?.unwrap_or_default();
    let mut ops = Vec::new();
    let mut handles = Vec::new();
    for (handle, node, wanted) in targets {
        let parent = parent_path(&node.path).and_then(|p| at_path(&tree, &p));
        let parent_placed = parent.as_ref().and_then(|(h, _)| placed.get(h));
        let types = parent_types(parent.as_ref().map(|(_, n)| n), parent_placed);
        let now = placed.get(&handle).map(|p| p.local);
        let bindings = place::bindings_for(&node, &types, now, wanted).map_err(|r| {
            let kind = if r.kind == "needs_preview" {
                "no_preview"
            } else {
                r.kind
            };
            refused(kind, r.message)
        })?;
        for (name, value) in bindings {
            ops.push(Op::SetBinding {
                node: Address::Node(handle.clone()),
                name,
                value,
            });
        }
        handles.push(handle);
    }
    Ok((ops, handles))
}

fn place_many(
    studio: &mut Studio,
    doc: &str,
    generation: Option<u64>,
    places: &[(Address, Wanted)],
    label: &str,
    dry_run: bool,
) -> Result<ManyOut, SurfaceError> {
    let (ops, nodes) = place_ops(studio, doc, places)?;
    let (outcome, _) = studio
        .session
        .edit_many(doc, generation, &ops, label, dry_run)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
    })
}

fn align(studio: &mut Studio, a: &AlignArgs) -> Result<ManyOut, SurfaceError> {
    if a.align.distributes() && a.nodes.len() < 3 {
        return Err(SurfaceError::invalid_argument(
            "distributing takes three objects or more: the outer two stay and those between move",
        ));
    }
    let found = geometry(
        studio,
        &GeometryArgs {
            doc: a.doc.clone(),
            nodes: a.nodes.clone(),
            generation: a.generation,
        },
    )?;
    let mut boxes = Vec::new();
    let mut locals = Vec::new();
    for node in &found.nodes {
        let (Some(window), Some(local)) = (node.window, node.local) else {
            return Err(refused(
                "not_shown",
                format!("the preview does not show `{}`", node.node),
            ));
        };
        boxes.push(window);
        locals.push(local);
    }
    let moves = place::aligned(&boxes, a.align, found.parent);
    let places: Vec<(Address, Wanted)> = found
        .nodes
        .iter()
        .zip(locals)
        .zip(moves)
        .map(|((node, local), (dx, dy))| {
            (
                Address::Node(node.node.clone()),
                Wanted {
                    x: (dx != 0.0).then_some(local.x + dx),
                    y: (dy != 0.0).then_some(local.y + dy),
                    ..Wanted::default()
                },
            )
        })
        .collect();
    place_many(studio, &a.doc, a.generation, &places, "align", false)
}

/// Each of `nodes` as written, with the ids in it.
fn copied(
    studio: &mut Studio,
    doc: &str,
    nodes: &[Address],
) -> Result<Vec<(String, Node, Copied)>, SurfaceError> {
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    let text = studio.session.text(doc).map_err(error)?.0.to_owned();
    let mut out = Vec::new();
    for address in nodes {
        let (handle, node) = studio.session.resolve(doc, address).map_err(error)?;
        let ids = tree
            .iter()
            .filter(|(_, n)| n.start >= node.start && n.end <= node.end)
            .filter_map(|(_, n)| n.id.clone())
            .collect();
        let copy = Copied {
            text: text[node.start..node.end].to_owned(),
            ids,
        };
        out.push((handle, node, copy));
    }
    Ok(out)
}

fn taken_ids(studio: &mut Studio, doc: &str) -> Result<Vec<String>, SurfaceError> {
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    Ok(tree.into_iter().filter_map(|(_, n)| n.id).collect())
}

/// `node` written with its literal x and y moved over, so the copy is seen.
fn shifted(node: &Node, text: &str) -> String {
    let mut spans: Vec<(usize, usize, f64)> = node
        .bindings
        .iter()
        .filter(|b| b.name == "x" || b.name == "y")
        .filter_map(|b| {
            let value = b.value.trim().parse::<f64>().ok()?;
            Some((b.start - node.start, b.end - node.start, value))
        })
        .collect();
    spans.sort_by_key(|(start, ..)| std::cmp::Reverse(*start));
    let mut out = text.to_owned();
    for (start, end, value) in spans {
        out.replace_range(start..end, &place::number(value + DUPLICATE_OFFSET));
    }
    out
}

fn duplicate(studio: &mut Studio, a: &NodesArgs) -> Result<ManyOut, SurfaceError> {
    let mut items = copied(studio, &a.doc, &a.nodes)?;
    let mut taken = taken_ids(studio, &a.doc)?;
    // Last in the file first: an insert only moves what comes after it, so
    // the places of those still to come stay true.
    items.sort_by_key(|(_, node, _)| std::cmp::Reverse(node.start));
    let (tree, _) = studio.session.tree(&a.doc).map_err(error)?;
    let mut ops = Vec::new();
    for (_, node, copy) in &items {
        let Some(parent) = parent_path(&node.path).and_then(|p| at_path(&tree, &p)) else {
            return Err(refused("root", "the document's root cannot be duplicated"));
        };
        let Step::Child(index) = node.path.0[node.path.0.len() - 1] else {
            return Err(refused(
                "not_a_child",
                format!(
                    "`{}` is the value of a property: there is no room beside it",
                    node.ty
                ),
            ));
        };
        let drag = place::drag_of(node, &[parent.1.ty.as_str()]);
        let text = if drag.moving == Moving::Position {
            shifted(node, &copy.text)
        } else {
            copy.text.clone()
        };
        ops.push(Op::Insert {
            parent: Address::Node(parent.0),
            index: Some(index + 1),
            text: place::with_fresh_ids(&text, &copy.ids, &mut taken),
            add_id: false,
            import: None,
        });
    }
    let (outcome, mut nodes) = studio
        .session
        .edit_many(&a.doc, a.generation, &ops, "duplicate", false)
        .map_err(error)?;
    nodes.reverse();
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
    })
}

fn paste(studio: &mut Studio, a: &PasteArgs) -> Result<ManyOut, SurfaceError> {
    if studio.clipboard.is_empty() {
        return Err(refused(
            "nothing_copied",
            "nothing is copied: `node_arrange` `copy` copies objects",
        ));
    }
    let mut taken = taken_ids(studio, &a.doc)?;
    let ops: Vec<Op> = studio
        .clipboard
        .iter()
        .map(|copy| Op::Insert {
            parent: a.parent.clone(),
            index: None,
            text: place::with_fresh_ids(&copy.text, &copy.ids, &mut taken),
            add_id: false,
            import: None,
        })
        .collect();
    let (outcome, nodes) = studio
        .session
        .edit_many(&a.doc, None, &ops, "paste", false)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
    })
}

fn needed<T>(field: Option<T>, name: &str, action: &str) -> Result<T, SurfaceError> {
    field.ok_or_else(|| SurfaceError::invalid_argument(format!("`{action}` needs `{name}`")))
}

fn arrange(studio: &mut Studio, a: ArrangeArgs) -> Result<ArrangeOut, SurfaceError> {
    let doc = a.doc;
    let generation = a.generation;
    let nodes = a.nodes;
    let many = |many: ManyOut| ArrangeOut {
        edit: Some(many.edit),
        nodes: many.nodes,
        ..ArrangeOut::default()
    };
    match a.action {
        Action::Geometry => {
            let found = geometry(
                studio,
                &GeometryArgs {
                    doc,
                    nodes,
                    generation,
                },
            )?;
            Ok(ArrangeOut {
                boxes: found.nodes,
                parent: found.parent,
                siblings: found.siblings,
                ..ArrangeOut::default()
            })
        }
        Action::Place => {
            let places: Vec<(Address, Wanted)> = needed(a.places, "places", "place")?
                .into_iter()
                .map(|p| {
                    (
                        p.node,
                        Wanted {
                            x: p.x,
                            y: p.y,
                            width: p.width,
                            height: p.height,
                        },
                    )
                })
                .collect();
            place_many(studio, &doc, generation, &places, "place", a.dry_run).map(many)
        }
        Action::Align => {
            let how = needed(a.align, "align", "align")?;
            align(
                studio,
                &AlignArgs {
                    doc,
                    nodes,
                    align: how,
                    generation,
                },
            )
            .map(many)
        }
        Action::Copy => {
            let items = copied(studio, &doc, &nodes)?;
            studio.clipboard = items.into_iter().map(|(_, _, copy)| copy).collect();
            Ok(ArrangeOut {
                copied: studio.clipboard.iter().map(|c| c.text.clone()).collect(),
                ..ArrangeOut::default()
            })
        }
        Action::Paste => {
            let parent = needed(a.parent, "parent", "paste")?;
            paste(studio, &PasteArgs { doc, parent }).map(many)
        }
        Action::Duplicate => duplicate(
            studio,
            &NodesArgs {
                doc,
                nodes,
                generation,
            },
        )
        .map(many),
    }
}

pub(crate) fn add(registry: ToolRegistry, studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let studio = studio.clone();
    registry.with(
        ToolDef::new::<ArrangeArgs>(
            "node_arrange",
            "What a canvas gesture does; each edit one undo step, all or none. `geometry`: boxes, \
             drag rules, siblings. `place`: x/y/size, writing what governs each object (anchor \
             margins, Layout.preferred*); refused `arranged` or `anchored` where overridden. \
             `align`, `copy`, `paste`, `duplicate`.",
        )
        .answers::<ArrangeOut>()
        .requires(Cap::Inspect)
        .annotated(MUTATE),
        typed(move |ctx: &CallCtx, a: ArrangeArgs| {
            // Reading and copying only read; the registry checked `inspect`,
            // the rest is checked here, refused the way the registry does.
            if !matches!(a.action, Action::Geometry | Action::Copy)
                && !ctx.caps.contains(Cap::Mutate)
            {
                return Err(denied("node_arrange", Cap::Mutate, ctx));
            }
            arrange(&mut lock(&studio), a)
        }),
    )
}
