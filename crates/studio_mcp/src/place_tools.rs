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
use studio_core::place::{self, Align, Drag, Moving, Rect, Resizing, Wanted, Wrap};
use studio_core::preview::{Declared, Placed};
use studio_core::scene3d::Handles;
use studio_core::{Address, Import, Node, NodePath, Op, StateEdit, Step};

use crate::tools::{EditOut, MUTATE};
use crate::{Studio, component, error, layout_tools, lock};

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
    /// The document it came from and what that imports, of which a paste into
    /// another document brings what it needs.
    from: String,
    imports: Vec<Import>,
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
    Remove,
    Rename,
    Relayout,
    Hide,
    Show,
    Lock,
    Unlock,
    Wrap,
    Unwrap,
    #[serde(rename = "component_extract")]
    ComponentExtract,
    Anchor,
}

// What `anchor` does to each side: a margin pins it, null takes it off. Said
// in `docs/MCP.md`, not here: the list of tools has a size it keeps to.
#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Sides {
    #[serde(default, deserialize_with = "stated")]
    left: Option<Option<f64>>,
    #[serde(default, deserialize_with = "stated")]
    right: Option<Option<f64>>,
    #[serde(default, deserialize_with = "stated")]
    top: Option<Option<f64>>,
    #[serde(default, deserialize_with = "stated")]
    bottom: Option<Option<f64>>,
}

/// Serde reads null as a field left out, and here they differ.
fn stated<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<f64>>, D::Error> {
    Option::<f64>::deserialize(d).map(Some)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArrangeArgs {
    action: Action,
    doc: String,
    /// For all but `place` and `paste`.
    #[serde(default)]
    nodes: Vec<Address>,
    /// For a `path`.
    generation: Option<u64>,
    /// For `geometry`: leave the siblings out (each is a read of the running app).
    #[serde(default)]
    without_siblings: bool,
    /// For `geometry`: the siblings are the first node's children, for something drawn inside it.
    #[serde(default)]
    within: bool,
    /// For `place`.
    places: Option<Vec<PlaceOne>>,
    /// For `place`, `align`: write into this state.
    record: Option<RecordInto>,
    /// For `align`; one object lines up with its parent.
    align: Option<Align>,
    /// For `paste`, into any document. Ids it has are renamed.
    parent: Option<Address>,
    /// For `rename`: the object's new `id`. For `component_extract`: the new type. For `relayout`: its new kind.
    name: Option<String>,
    /// For `wrap`.
    #[serde(rename = "in")]
    wrap_in: Option<Wrap>,
    /// For `wrap`.
    spacing: Option<f64>,
    /// For `wrap` in a Grid.
    columns: Option<u32>,
    /// For `anchor`.
    fill: Option<bool>,
    /// For `anchor`.
    #[serde(rename = "centerIn")]
    center_in: Option<bool>,
    /// For `anchor`: null unpins.
    sides: Option<Sides>,
    /// For `anchor`: `parent` or a sibling's id.
    target: Option<String>,
    #[serde(default)]
    dry_run: bool,
}

// The state a gesture is recorded into, and the object that holds it.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RecordInto {
    state: String,
    on: Address,
}

#[derive(Default, Serialize, JsonSchema)]
struct ArrangeOut {
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    edit: Option<EditOut>,
    /// In the order asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<String>,
    /// `geometry`: each object asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    boxes: Vec<NodeGeometry>,
    /// `geometry`: the first one's parent, in the window.
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<Rect>,
    /// `geometry`: the parent's other children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    siblings: Vec<Sibling>,
    /// `copy`: each object as written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    copied: Vec<String>,
    /// `component_extract`: the file it wrote.
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<String>,
    /// `component_extract`: what outside reads of the inside became.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    /// `component_extract`: Studio cannot tell whether an import has a type of
    /// this name. If one has, the instance is that type, and not the new file.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    name_unsure: bool,
    /// `paste`: what it imported for what it holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    imported: Vec<String>,
    /// `paste`: each name it could not show still lands where it did: written
    /// anyway, for the agent to look at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unvouched: Vec<crate::signals::Unvouched>,
}

struct GeometryArgs {
    doc: String,
    nodes: Vec<Address>,
    generation: Option<u64>,
    siblings: bool,
    within: bool,
}

struct GeometryOut {
    nodes: Vec<NodeGeometry>,
    parent: Option<Rect>,
    siblings: Vec<Sibling>,
}

#[derive(Serialize, JsonSchema)]
struct NodeGeometry {
    node: String,
    /// In the parent's coordinates; absent when the preview has no such object.
    #[serde(skip_serializing_if = "Option::is_none")]
    local: Option<Rect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<Rect>,
    drag: Drag,
    /// An object of a 3D scene has no box: its axes on the view instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    scene: Option<Spatial>,
}

/// A model, a light or a camera on the picture of its view, for the canvas to
/// draw arrows on and drag along.
#[derive(Serialize, JsonSchema)]
struct Spatial {
    #[serde(flatten)]
    handles: Handles,
    /// `x`, `y` and `z` as the file has them.
    #[schemars(with = "Vec<f64>")]
    local: [f64; 3],
    /// Per axis, why a drag cannot write it; empty where it can.
    #[schemars(with = "Vec<String>")]
    refused: [String; 3],
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
    /// Of an object of a 3D scene.
    z: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
}

pub(crate) struct ManyOut {
    pub(crate) edit: EditOut,
    pub(crate) nodes: Vec<String>,
    /// What a paste imported.
    pub(crate) imported: Vec<String>,
    /// What a paste could not show still lands where it did.
    pub(crate) unvouched: Vec<crate::signals::Unvouched>,
}

struct AlignArgs {
    doc: String,
    nodes: Vec<Address>,
    align: Align,
    generation: Option<u64>,
    record: Option<RecordInto>,
    dry_run: bool,
}

struct NodesArgs {
    doc: String,
    nodes: Vec<Address>,
    generation: Option<u64>,
    dry_run: bool,
}

struct PasteArgs {
    doc: String,
    parent: Address,
    generation: Option<u64>,
    dry_run: bool,
}

pub(crate) fn refused(kind: &str, message: impl Into<String>) -> SurfaceError {
    SurfaceError::new(ErrorCode::NotActionable, message).with_details(json!({ "kind": kind }))
}

pub(crate) fn parent_path(path: &NodePath) -> Option<NodePath> {
    (!path.0.is_empty()).then(|| NodePath(path.0[..path.0.len() - 1].to_vec()))
}

pub(crate) fn short(ty: &str) -> &str {
    ty.rsplit('.').next().unwrap_or(ty)
}

/// The objects the preview of `doc` has of `nodes`, by handle; the first copy
/// of a delegate speaks for all, since a file has one declaration of them.
/// `None` when no preview of it is showing.
pub(crate) fn placed_of(
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
pub(crate) fn at_path(tree: &[(String, Node)], path: &NodePath) -> Option<(String, Node)> {
    tree.iter().find(|(_, n)| n.path == *path).cloned()
}

/// The types of what places `node`: the file's, and the preview's too when it
/// is known, since a component of the project can be a layout underneath.
pub(crate) fn parent_types<'a>(
    parent: Option<&'a Node>,
    placed: Option<&'a Placed>,
) -> Vec<&'a str> {
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
    let around = if a.within {
        asked.first().cloned()
    } else {
        first_parent.clone()
    };
    let siblings: Vec<(String, Node)> = around
        .iter()
        .flat_map(|(_, parent)| {
            tree.iter().filter(move |(_, n)| {
                parent_path(&n.path).as_ref() == Some(&parent.path)
                    && matches!(n.path.0.last(), Some(Step::Child(_)))
            })
        })
        .filter(|(handle, _)| !asked.iter().any(|(h, _)| h == handle))
        .take(if a.siblings { MOST_SIBLINGS } else { 0 })
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
    let mut nodes = Vec::new();
    for (handle, node) in &asked {
        let parent = parent_path(&node.path).and_then(|p| at_path(&tree, &p));
        let parent_placed = parent.as_ref().and_then(|(h, _)| placed.get(h));
        let mut drag = place::drag_of(
            node,
            &parent_types(parent.as_ref().map(|(_, n)| n), parent_placed),
        );
        let here = placed.get(handle);
        let scene = match here {
            None if place::is_spatial(&node.ty) => spatial(studio, &a.doc, node)?,
            _ => None,
        };
        if scene.is_some() {
            drag = Drag {
                moving: Moving::Position,
                resize: Resizing::None,
                says: String::new(),
            };
        }
        nodes.push(NodeGeometry {
            node: handle.clone(),
            local: here.map(|p| as_written(node, &drag, p.local)),
            window: here.map(|p| p.window),
            drag,
            scene,
        });
    }
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

/// `node` of a 3D scene on the picture of its view; `None` where the preview
/// does not show it (a model with no mesh to see, a camera it is not looking
/// through, a preview that cannot read a scene).
fn spatial(studio: &mut Studio, doc: &str, node: &Node) -> Result<Option<Spatial>, SurfaceError> {
    let (line, column) = studio.session.declared_where(doc, node).map_err(error)?;
    let at = Declared {
        file: doc.to_owned(),
        line: line as u32,
        column: column as u32,
    };
    let Some(scene) = crate::preview_tools::scene(studio, doc, &at)? else {
        return Ok(None);
    };
    let Some(handles) = scene.body(&at).and_then(|b| scene.handles(b)) else {
        return Ok(None);
    };
    let written = |name: &str| node.bindings.iter().find(|b| b.name == name);
    let whole = written("position").is_some();
    let mut local = [0.0; 3];
    let mut refused = [String::new(), String::new(), String::new()];
    for (i, name) in ["x", "y", "z"].into_iter().enumerate() {
        match written(name).map(|b| b.value.trim()) {
            None => {}
            Some(v) => match v.parse() {
                Ok(n) => local[i] = n,
                Err(_) => refused[i] = format!("{name} is `{v}`: an expression"),
            },
        }
        if whole {
            refused[i] = "position is set whole".to_owned();
        }
    }
    Ok(Some(Spatial {
        handles,
        local,
        refused,
    }))
}

/// The edits placing each of `places` asks for, all or none.
fn place_ops(
    studio: &mut Studio,
    doc: &str,
    places: &[(Address, Wanted)],
    record: Option<&RecordInto>,
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
        let placing = if place::is_spatial(&node.ty) {
            place::bindings_3d(&node, wanted)
        } else if wanted.z.is_some() {
            return Err(SurfaceError::invalid_argument(format!(
                "`z` is for an object of a 3D scene, and `{}` is a {}",
                node.id.as_deref().unwrap_or(&node.ty),
                node.ty
            )));
        } else {
            place::bindings_for(&node, &types, now, wanted)
        };
        let bindings = placing.map_err(|r| {
            let kind = if r.kind == "needs_preview" {
                "no_preview"
            } else {
                r.kind
            };
            refused(kind, r.message)
        })?;
        for (name, value) in bindings {
            ops.push(match record {
                // What a state changes is written with the object's id.
                Some(record) => Op::State {
                    node: record.on.clone(),
                    edit: StateEdit::Change {
                        name: record.state.clone(),
                        target: node.id.clone().ok_or_else(|| {
                            error(studio_core::Error::NoId(format!(
                                "{handle} (`{}`)",
                                node.ty
                            )))
                        })?,
                        property: name,
                        value: Some(value),
                    },
                },
                None => Op::SetBinding {
                    node: Address::Node(handle.clone()),
                    name,
                    value,
                },
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
    record: Option<&RecordInto>,
    dry_run: bool,
) -> Result<ManyOut, SurfaceError> {
    let (ops, nodes) = place_ops(studio, doc, places, record)?;
    let (outcome, _) = studio
        .session
        .edit_many(doc, generation, &ops, label, dry_run)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
        imported: Vec::new(),
        unvouched: Vec::new(),
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
            siblings: false,
            within: false,
        },
    )?;
    let mut boxes = Vec::new();
    let mut locals = Vec::new();
    for node in &found.nodes {
        let (Some(window), Some(local)) = (node.window, node.local) else {
            return Err(refused(
                "not_shown",
                format!("the preview has no box for `{}`", node.node),
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
    place_many(
        studio,
        &a.doc,
        a.generation,
        &places,
        "align",
        a.record.as_ref(),
        a.dry_run,
    )
}

/// Each of `nodes` as written, with the ids in it.
fn copied(
    studio: &mut Studio,
    doc: &str,
    nodes: &[Address],
) -> Result<Vec<(String, Node, Copied)>, SurfaceError> {
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    let text = studio.session.text(doc).map_err(error)?.0.to_owned();
    let imports = studio_core::outline::imports(&text).unwrap_or_default();
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
            from: doc.to_owned(),
            imports: imports.clone(),
        };
        out.push((handle, node, copy));
    }
    Ok(out)
}

pub(crate) fn taken_ids(studio: &mut Studio, doc: &str) -> Result<Vec<String>, SurfaceError> {
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
        .edit_many(&a.doc, a.generation, &ops, "duplicate", a.dry_run)
        .map_err(error)?;
    nodes.reverse();
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
        imported: Vec::new(),
        unvouched: Vec::new(),
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
    let source = studio.session.text(&a.doc).map_err(error)?.0.to_owned();
    // All at once and in the source's order: one copy is one document's, and
    // a folder one object brings can take another object's name.
    let texts: Vec<&str> = studio.clipboard.iter().map(|c| c.text.as_str()).collect();
    let first = &studio.clipboard[0];
    // Bare, even beside an import under a name: nothing has to qualify what
    // the text holds.
    let crate::loads::Pasted {
        imports: imported,
        unvouched,
    } = crate::loads::imports_for(
        studio.session.store(),
        &a.doc,
        &source,
        &texts,
        &first.from,
        &first.imports,
    )?;
    let mut ops: Vec<Op> = imported
        .iter()
        .map(|module| Op::Import {
            module: module.clone(),
        })
        .collect();
    for copy in &studio.clipboard {
        ops.push(Op::Insert {
            parent: a.parent.clone(),
            index: None,
            text: place::with_fresh_ids(&copy.text, &copy.ids, &mut taken),
            add_id: false,
            import: None,
        });
    }
    let (outcome, nodes) = studio
        .session
        .edit_many(&a.doc, a.generation, &ops, "paste", a.dry_run)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
        imported,
        unvouched,
    })
}

/// Every object of `a` gone in one step. Handles first: taking one out moves
/// the paths of those after it. An object inside another of them goes with
/// it, and has no edit of its own: by then it would not be there to take out.
fn remove(studio: &mut Studio, a: &NodesArgs) -> Result<ManyOut, SurfaceError> {
    let mut found: Vec<(String, std::ops::Range<usize>)> = Vec::new();
    for address in &a.nodes {
        let (handle, node) = studio.session.resolve(&a.doc, address).map_err(error)?;
        found.push((handle, node.start..node.end));
    }
    let mut ops = Vec::new();
    let mut nodes = Vec::new();
    for (i, (handle, span)) in found.iter().enumerate() {
        let inside = found.iter().enumerate().any(|(j, (_, other))| {
            let same = other == span;
            other.start <= span.start && span.end <= other.end && (!same || j < i)
        });
        if inside {
            continue;
        }
        ops.push(Op::Remove {
            node: Address::Node(handle.clone()),
        });
        nodes.push(handle.clone());
    }
    let (outcome, _) = studio
        .session
        .edit_many(&a.doc, a.generation, &ops, "remove", a.dry_run)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
        imported: Vec::new(),
        unvouched: Vec::new(),
    })
}

/// A write turns addresses into handles before the session sees them, so the
/// session's rule for a path is kept here: it is true of the text it was read
/// from, and is refused on the generation alone, never rebased.
fn check_paths(studio: &mut Studio, a: &ArrangeArgs) -> Result<(), SurfaceError> {
    if matches!(
        a.action,
        Action::Geometry
            | Action::Copy
            | Action::Hide
            | Action::Show
            | Action::Lock
            | Action::Unlock
    ) {
        return Ok(());
    }
    let placed = a.places.iter().flatten().map(|p| &p.node);
    let path = a
        .nodes
        .iter()
        .chain(placed)
        .find_map(|address| match address {
            Address::Path(path) => Some(path),
            _ => None,
        });
    let Some(path) = path else {
        return Ok(());
    };
    let Some(given) = a.generation else {
        return Err(SurfaceError::invalid_argument(format!(
            "the path `{path}` names an object only in the generation it was read at: give \
             `generation`"
        )));
    };
    let current = studio.session.generation(&a.doc).map_err(error)?;
    if given != current {
        return Err(error(studio_core::Error::Stale {
            doc: a.doc.clone(),
            given,
            current,
        }));
    }
    Ok(())
}

fn needed<T>(field: Option<T>, name: &str, action: &str) -> Result<T, SurfaceError> {
    field.ok_or_else(|| SurfaceError::invalid_argument(format!("`{action}` needs `{name}`")))
}

fn arrange(studio: &mut Studio, a: ArrangeArgs) -> Result<ArrangeOut, SurfaceError> {
    check_paths(studio, &a)?;
    if matches!(
        a.action,
        Action::Place
            | Action::Align
            | Action::Remove
            | Action::Wrap
            | Action::Unwrap
            | Action::Anchor
            | Action::Relayout
            | Action::ComponentExtract
    ) {
        check_unlocked(studio, &a)?;
    }
    let doc = a.doc;
    let generation = a.generation;
    let nodes = a.nodes;
    let many = |many: ManyOut| ArrangeOut {
        edit: Some(many.edit),
        nodes: many.nodes,
        imported: many.imported,
        unvouched: many.unvouched,
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
                    siblings: !a.without_siblings,
                    within: a.within,
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
                            z: p.z,
                            width: p.width,
                            height: p.height,
                        },
                    )
                })
                .collect();
            place_many(
                studio,
                &doc,
                generation,
                &places,
                "place",
                a.record.as_ref(),
                a.dry_run,
            )
            .map(many)
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
                    record: a.record,
                    dry_run: a.dry_run,
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
            paste(
                studio,
                &PasteArgs {
                    doc,
                    parent,
                    generation,
                    dry_run: a.dry_run,
                },
            )
            .map(many)
        }
        Action::Rename => {
            let name = needed(a.name, "name", "rename")?;
            rename(studio, &doc, generation, &nodes, &name, a.dry_run)
        }
        Action::Relayout => {
            let kind = needed(a.name, "name", "relayout")?;
            relayout(studio, &doc, generation, &nodes, &kind, a.dry_run)
        }
        Action::Hide | Action::Show => {
            let objects = objects_with_ids(studio, &doc, &nodes)?;
            if a.dry_run {
                return Ok(ArrangeOut::default());
            }
            let mut previews = crate::preview::lock(&studio.previews);
            for (id, handle, node) in objects {
                let pairs = studio
                    .rehandled
                    .iter()
                    .map(|(o, n)| (o.as_str(), n.as_str()));
                let handles = crate::linked(pairs, &handle);
                if matches!(a.action, Action::Hide) {
                    previews.hide(&doc, handles, id);
                } else {
                    let written = node.bindings.iter().find(|b| b.name == "visible");
                    let visible = written.is_none_or(|b| b.value.trim() != "false");
                    previews.show(&doc, handles, &id, visible);
                }
            }
            drop(previews);
            studio.watchers.tell();
            Ok(ArrangeOut::default())
        }
        Action::Lock | Action::Unlock => {
            let mut handles = Vec::new();
            for address in &nodes {
                handles.push(studio.session.resolve(&doc, address).map_err(error)?.0);
            }
            if a.dry_run {
                return Ok(ArrangeOut::default());
            }
            for handle in handles {
                let pairs = studio
                    .rehandled
                    .iter()
                    .map(|(o, n)| (o.as_str(), n.as_str()));
                for handle in crate::linked(pairs, &handle) {
                    if matches!(a.action, Action::Lock) {
                        studio.locked.insert(handle);
                    } else {
                        studio.locked.remove(&handle);
                    }
                }
            }
            studio.watchers.tell();
            Ok(ArrangeOut::default())
        }
        Action::Wrap => {
            let how = needed(a.wrap_in, "in", "wrap")?;
            layout_tools::wrap(
                studio,
                &layout_tools::WrapArgs {
                    doc,
                    nodes,
                    how,
                    spacing: a.spacing,
                    columns: a.columns,
                    generation,
                    dry_run: a.dry_run,
                },
            )
            .map(many)
        }
        Action::ComponentExtract => {
            let name = needed(a.name, "name", "component_extract")?;
            let [node] = nodes.as_slice() else {
                return Err(SurfaceError::invalid_argument(
                    "`component_extract` takes one object",
                ));
            };
            let made = component::extract(studio, &doc, node, &name, generation, a.dry_run)?;
            Ok(ArrangeOut {
                edit: Some(made.edit),
                component: Some(made.component),
                aliases: made.aliases,
                name_unsure: made.name_unsure,
                ..ArrangeOut::default()
            })
        }
        Action::Unwrap => {
            layout_tools::unwrap(studio, &doc, &nodes, generation, a.dry_run).map(many)
        }
        Action::Anchor => {
            let sides = a.sides.unwrap_or_default();
            let pin = place::Pin {
                fill: a.fill,
                center_in: a.center_in,
                left: sides.left,
                right: sides.right,
                top: sides.top,
                bottom: sides.bottom,
                target: a.target.unwrap_or_else(|| "parent".to_owned()),
            };
            layout_tools::anchor(studio, &doc, &nodes, &pin, generation, a.dry_run).map(many)
        }
        Action::Duplicate | Action::Remove => {
            let args = NodesArgs {
                doc,
                nodes,
                generation,
                dry_run: a.dry_run,
            };
            if matches!(a.action, Action::Remove) {
                remove(studio, &args).map(many)
            } else {
                duplicate(studio, &args).map(many)
            }
        }
    }
}

/// Refused whoever asks: a lock that an agent's `place` walked through
/// would protect nothing a person left it to protect.
fn check_unlocked(studio: &mut Studio, a: &ArrangeArgs) -> Result<(), SurfaceError> {
    if studio.locked.is_empty() {
        return Ok(());
    }
    let placed = a.places.iter().flatten().map(|p| &p.node);
    for address in a.nodes.iter().chain(placed) {
        let (handle, _) = studio.session.resolve(&a.doc, address).map_err(error)?;
        if studio.locked.contains(&handle) {
            return Err(refused(
                "locked",
                format!("`{handle}` is locked: `unlock` it first"),
            ));
        }
    }
    Ok(())
}

/// The preview finds an object by its `id`; one without has nothing to find.
fn objects_with_ids(
    studio: &mut Studio,
    doc: &str,
    nodes: &[Address],
) -> Result<Vec<(String, String, Node)>, SurfaceError> {
    let mut objects = Vec::new();
    for address in nodes {
        let (handle, node) = studio.session.resolve(doc, address).map_err(error)?;
        let Some(id) = node.id.clone() else {
            return Err(refused(
                "no_id",
                format!(
                    "this `{}` has no id for the preview to find it by: give it an id",
                    node.ty
                ),
            ));
        };
        objects.push((id, handle, node));
    }
    Ok(objects)
}

/// The kinds a layout can turn into: what places a child by its own numbers
/// never turns into what places it by `Layout.*`, which would move it.
const LAYOUT_FAMILIES: [&[&str]; 2] = [
    &["Row", "Column", "Grid", "Flow"],
    &["RowLayout", "ColumnLayout", "GridLayout"],
];

fn relayout(
    studio: &mut Studio,
    doc: &str,
    generation: Option<u64>,
    nodes: &[Address],
    kind: &str,
    dry_run: bool,
) -> Result<ArrangeOut, SurfaceError> {
    let [address] = nodes else {
        return Err(refused("one_object", "`relayout` turns one object"));
    };
    let (handle, node) = studio.session.resolve(doc, address).map_err(error)?;
    let family = |ty: &str| LAYOUT_FAMILIES.iter().find(|f| f.contains(&ty));
    let (Some(from), Some(to)) = (family(&node.ty), family(kind)) else {
        return Err(refused(
            "not_a_layout",
            format!(
                "`relayout` turns a Row, Column, Grid, Flow or `*Layout` into another: not `{}` into `{kind}`",
                node.ty
            ),
        ));
    };
    if from != to {
        return Err(refused(
            "other_family",
            format!(
                "a `{}` places its children by their numbers or `Layout.*`, a `{kind}` by the other: wrap them anew instead",
                node.ty
            ),
        ));
    }
    let source = studio.session.text(doc).map_err(error)?.0.to_owned();
    let others: Vec<&str> = crate::signals::Names::of(studio.session.store(), doc, &source, &[])
        .map(|names| {
            node.bindings
                .iter()
                .filter(|b| names.lacks(kind, Some(&node), &b.name))
                .map(|b| b.name.as_str())
                .collect()
        })
        .unwrap_or_default();
    if !others.is_empty() {
        return Err(refused(
            "says_other",
            format!(
                "a `{kind}` has no `{}`: take it away first",
                others.join("`, `")
            ),
        ));
    }
    let text = studio.session.text(doc).map_err(error)?.0.to_owned();
    let text = format!(
        "{}{kind}{}",
        &text[..node.start],
        &text[node.start + node.ty.len()..]
    );
    let selected = studio
        .session
        .selection()
        .contains(&(doc.to_owned(), handle.clone()));
    let outcome = studio
        .session
        .replace_text(doc, generation, None, &text, dry_run)
        .map_err(error)?;
    if dry_run {
        return Ok(ArrangeOut {
            edit: Some(outcome.into()),
            nodes: vec![handle],
            ..ArrangeOut::default()
        });
    }
    // Its place in the file is all that hasn't changed.
    let at = Address::Path(node.path.to_string());
    let now = rehandled(studio, doc, &handle, selected, at)?;
    Ok(ArrangeOut {
        edit: Some(outcome.into()),
        nodes: vec![now],
        ..ArrangeOut::default()
    })
}

fn rename(
    studio: &mut Studio,
    doc: &str,
    generation: Option<u64>,
    nodes: &[Address],
    name: &str,
    dry_run: bool,
) -> Result<ArrangeOut, SurfaceError> {
    let [address] = nodes else {
        return Err(refused("one_object", "`rename` names one object"));
    };
    let lowercase = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    if !lowercase || !studio_core::flows::is_identifier(name) {
        return Err(refused(
            "not_an_id",
            format!("`{name}` is not an id: an id is one word and starts lowercase"),
        ));
    }
    if crate::signals::not_an_id(name) {
        return Err(refused(
            "not_an_id",
            format!("`{name}` is not an id: QML keeps the word for itself"),
        ));
    }
    if taken_ids(studio, doc)?.iter().any(|id| id == name) {
        return Err(refused(
            "id_taken",
            format!("an object of `{doc}` already has the id `{name}`"),
        ));
    }
    let (handle, node) = studio.session.resolve(doc, address).map_err(error)?;
    let selected = studio
        .session
        .selection()
        .contains(&(doc.to_owned(), handle.clone()));
    let outcome = match &node.id {
        Some(old) => {
            // Nothing outside the file reads an id, so the file is all there is.
            let text = studio.session.text(doc).map_err(error)?.0.to_owned();
            let text = place::with_id_renamed(&text, old, name);
            studio
                .session
                .replace_text(doc, generation, None, &text, dry_run)
        }
        None => {
            let text = studio.session.text(doc).map_err(error)?.0.to_owned();
            let object =
                studio_core::with_first_id(&text[node.start..node.end], name).map_err(error)?;
            let text = format!("{}{object}{}", &text[..node.start], &text[node.end..]);
            studio
                .session
                .replace_text(doc, generation, None, &text, dry_run)
        }
    }
    .map_err(error)?;
    if dry_run {
        return Ok(ArrangeOut {
            edit: Some(outcome.into()),
            nodes: vec![handle],
            ..ArrangeOut::default()
        });
    }
    let now = rehandled(studio, doc, &handle, selected, Address::Id(name.to_owned()))?;
    Ok(ArrangeOut {
        edit: Some(outcome.into()),
        nodes: vec![now],
        ..ArrangeOut::default()
    })
}

/// Written as a new text, an object comes back with a new handle: what Studio
/// holds of the old one goes with it. Gives the new one.
fn rehandled(
    studio: &mut Studio,
    doc: &str,
    handle: &str,
    selected: bool,
    address: Address,
) -> Result<String, SurfaceError> {
    let (now, _) = studio.session.resolve(doc, &address).map_err(error)?;
    if studio.locked.contains(handle) {
        studio.locked.insert(now.clone());
    }
    studio.rehandled.push((handle.to_owned(), now.clone()));
    if selected {
        let others: Vec<Address> = studio
            .session
            .selection()
            .into_iter()
            .filter(|(_, h)| h != handle)
            .map(|(_, h)| Address::Node(h))
            .chain([address])
            .collect();
        studio.session.select(doc, &others).map_err(error)?;
    }
    // The call's end writes it under its new id (`hidden_now`).
    let mut previews = crate::preview::lock(&studio.previews);
    if previews.is_hidden(doc, handle) {
        previews.hidden.insert((doc.to_owned(), now.clone()));
    }
    drop(previews);
    Ok(now)
}

pub(crate) fn add(registry: ToolRegistry, studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let studio = studio.clone();
    registry.with(
        ToolDef::new::<ArrangeArgs>(
            "node_arrange",
            "A canvas gesture, one undo step. `geometry`: boxes, drag rules, siblings. `place`: x/y/size, writing what governs each object. `component_extract`: one object to `name`.qml. `hide`, `show`, `lock`, `unlock` leave the file alone.",
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
