//! Putting objects in a layout and taking them out, and pinning them with
//! anchors. Each is one undo step; what the moved objects say stays as written.

use std::collections::HashMap;

use ljos_mcp_core::SurfaceError;
use studio_core::place::{self, Change, Known, Pin, Wrap};
use studio_core::preview::Placed;
use studio_core::{Address, Node, Op, Step};

use crate::place_tools::{
    ManyOut, parent_path, parent_types, placed_of, refused, short, taken_ids,
};
use crate::{Studio, error};

/// What only places or groups what is in it, so that nothing but a place is
/// lost when it goes. Anything else draws or does something of its own.
const WRAPPERS: [&str; 8] = [
    "Row",
    "Column",
    "Grid",
    "Flow",
    "Item",
    "RowLayout",
    "ColumnLayout",
    "GridLayout",
];

pub(crate) struct WrapArgs {
    pub(crate) doc: String,
    pub(crate) nodes: Vec<Address>,
    pub(crate) how: Wrap,
    pub(crate) spacing: Option<f64>,
    pub(crate) columns: Option<u32>,
    pub(crate) generation: Option<u64>,
    pub(crate) dry_run: bool,
}

fn label(node: &Node) -> &str {
    node.id.as_deref().unwrap_or(&node.ty)
}

fn written<'n>(node: &'n Node, name: &str) -> Option<&'n str> {
    node.bindings
        .iter()
        .find(|b| b.name == name)
        .map(|b| b.value.trim())
}

/// The box of `node` as far as it is known. What the file says of a plain
/// number goes before the preview: one that has not reloaded yet still shows
/// where it used to be.
fn known(node: &Node, placed: Option<&Placed>) -> Known {
    let free = |name: &str, live: Option<f64>, absent: Option<f64>| match written(node, name) {
        Some(value) => value.parse().ok().or(live),
        None => live.or(absent),
    };
    let local = placed.map(|p| p.local);
    Known {
        x: free("x", local.map(|r| r.x), Some(0.0)),
        y: free("y", local.map(|r| r.y), Some(0.0)),
        width: free("width", local.map(|r| r.width), None),
        height: free("height", local.map(|r| r.height), None),
    }
}

/// A word for a new object's id: the type's, numbered once it is taken.
pub(crate) fn fresh_id(stem: &str, taken: &[String]) -> String {
    std::iter::once(stem.to_owned())
        .chain((1..).map(|n| format!("{stem}{n}")))
        .find(|name| !taken.contains(name))
        .expect("an unused name")
}

/// What `who` reads of its parent means something else in another parent.
fn reads_parent(node: &Node) -> Option<&str> {
    node.bindings
        .iter()
        .find(|b| place::reads(&b.value, "parent"))
        .map(|b| b.name.as_str())
}

fn check_may_leave_parent(node: &Node) -> Result<(), SurfaceError> {
    if let Some(name) = reads_parent(node) {
        return Err(refused(
            "reads_parent",
            format!(
                "`{}`'s {name} reads `parent`, which would become another object: bind it to an \
                 id first",
                label(node)
            ),
        ));
    }
    Ok(())
}

pub(crate) fn wrap(studio: &mut Studio, a: &WrapArgs) -> Result<ManyOut, SurfaceError> {
    if a.columns.is_some() && a.how != Wrap::Grid {
        return Err(SurfaceError::invalid_argument("only a Grid has `columns`"));
    }
    if a.nodes.is_empty() {
        return Err(SurfaceError::invalid_argument("`wrap` needs `nodes`"));
    }
    let (tree, _) = studio.session.tree(&a.doc).map_err(error)?;
    let mut found: Vec<(String, Node)> = Vec::new();
    for address in &a.nodes {
        let one = studio.session.resolve(&a.doc, address).map_err(error)?;
        if !found.iter().any(|(h, _)| *h == one.0) {
            found.push(one);
        }
    }
    found.sort_by_key(|(_, node)| node.start);
    let at = |path| tree.iter().find(|(_, n)| n.path == path).cloned();
    let Some(parent) = parent_path(&found[0].1.path).and_then(at) else {
        return Err(refused("root", "the document's root has nothing around it"));
    };
    for (_, node) in &found {
        if parent_path(&node.path).as_ref() != Some(&parent.1.path)
            || !matches!(node.path.0.last(), Some(Step::Child(_)))
        {
            return Err(refused(
                "not_siblings",
                format!(
                    "`{}` is not a child of `{}`: a wrap takes siblings",
                    label(node),
                    label(&parent.1)
                ),
            ));
        }
        if place::anchored(node) {
            return Err(refused(
                "anchored",
                format!(
                    "`{}` is anchored, which a new parent would break: take its anchors off first",
                    label(node)
                ),
            ));
        }
        check_may_leave_parent(node)?;
    }
    // A sibling that stays can't anchor to what is now inside another object.
    let moved_ids: Vec<&str> = found.iter().filter_map(|(_, n)| n.id.as_deref()).collect();
    for (handle, stays) in &tree {
        if parent_path(&stays.path).as_ref() != Some(&parent.1.path)
            || found.iter().any(|(h, _)| h == handle)
        {
            continue;
        }
        for b in stays
            .bindings
            .iter()
            .filter(|b| b.name.starts_with("anchors."))
        {
            if let Some(id) = moved_ids.iter().find(|id| place::reads(&b.value, id)) {
                return Err(refused(
                    "anchored_to",
                    format!(
                        "`{}`'s {} reads `{id}`, which would no longer be its sibling: take that \
                         anchor off first",
                        label(stays),
                        b.name
                    ),
                ));
            }
        }
    }
    let index = match found[0].1.path.0.last() {
        Some(Step::Child(index)) => *index,
        _ => 0,
    };
    let mut wanted = found.clone();
    wanted.push(parent.clone());
    let placed = placed_of(studio, &a.doc, &wanted)?.unwrap_or_default();
    let boxes: Vec<Known> = found
        .iter()
        .map(|(handle, node)| known(node, placed.get(handle)))
        .collect();
    let plan = place::wrap_plan(&boxes, a.how, a.spacing, a.columns);
    let types = parent_types(Some(&parent.1), placed.get(&parent.0));
    let placed_by_parent = place::placer(&types).is_some();

    let (ty, stem) = match a.how {
        Wrap::Row => ("Row", "row"),
        Wrap::Column => ("Column", "column"),
        Wrap::Grid => ("Grid", "grid"),
        Wrap::Item => ("Item", "item"),
    };
    let id = fresh_id(stem, &taken_ids(studio, &a.doc)?);
    let mut lines = vec![format!("id: {id}")];
    let mut line = |name: &str, value: Option<String>| {
        if let Some(value) = value {
            lines.push(format!("{name}: {value}"));
        }
    };
    if !placed_by_parent {
        line("x", plan.x.map(place::number));
        line("y", plan.y.map(place::number));
    }
    line("width", plan.width.map(place::number));
    line("height", plan.height.map(place::number));
    line("columns", plan.columns.map(|c| c.to_string()));
    line("spacing", plan.spacing.map(place::number));
    let text = format!("{ty} {{\n    {}\n}}", lines.join("\n    "));

    let container = Address::Id(id);
    let mut ops = vec![Op::Insert {
        parent: Address::Node(parent.0),
        index: Some(index),
        text,
        add_id: false,
        import: Some("QtQuick".to_owned()),
    }];
    for &i in &plan.order {
        let (handle, node) = &found[i];
        let address = Address::Node(handle.clone());
        ops.push(Op::Move {
            node: address.clone(),
            parent: container.clone(),
            index: None,
        });
        if a.how == Wrap::Item {
            // Where it is on the screen stays, counted from the Item's corner.
            ops.extend(moved_by(&address, node, "x", plan.x));
            ops.extend(moved_by(&address, node, "y", plan.y));
        } else {
            for name in ["x", "y"] {
                if written(node, name).is_some() {
                    ops.push(Op::RemoveBinding {
                        node: address.clone(),
                        name: name.to_owned(),
                    });
                }
            }
        }
    }
    let (outcome, nodes) = studio
        .session
        .edit_many(&a.doc, a.generation, &ops, "wrap", a.dry_run)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes,
        imported: Vec::new(),
    })
}

fn moved_by(address: &Address, node: &Node, name: &str, origin: Option<f64>) -> Option<Op> {
    let origin = origin.filter(|o| *o != 0.0)?;
    Some(Op::SetBinding {
        node: address.clone(),
        name: name.to_owned(),
        value: place::moved(Some(written(node, name).unwrap_or("0")), -origin),
    })
}

/// Where each of `children` is in what `outer` holds, by the preview.
fn shown_in(
    placed: &HashMap<String, Placed>,
    handle: &str,
    outer: Option<&Placed>,
) -> Option<(f64, f64)> {
    let (one, outer) = (placed.get(handle)?, outer?);
    Some((one.window.x - outer.window.x, one.window.y - outer.window.y))
}

pub(crate) fn unwrap(
    studio: &mut Studio,
    doc: &str,
    nodes: &[Address],
    generation: Option<u64>,
    dry_run: bool,
) -> Result<ManyOut, SurfaceError> {
    let [address] = nodes else {
        return Err(refused("one_object", "`unwrap` names one object"));
    };
    let (handle, wrapper) = studio.session.resolve(doc, address).map_err(error)?;
    if !WRAPPERS.contains(&short(&wrapper.ty)) {
        return Err(refused(
            "not_a_layout",
            format!(
                "`{}` is a {}, which is more than what holds its children: `node_remove` would \
                 lose it",
                label(&wrapper),
                wrapper.ty
            ),
        ));
    }
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    let at = |path| tree.iter().find(|(_, n)| n.path == path).cloned();
    let Some(outer) = parent_path(&wrapper.path).and_then(at) else {
        return Err(refused("root", "the document's root has nothing to go to"));
    };
    let Some(&Step::Child(index)) = wrapper.path.0.last() else {
        return Err(refused(
            "not_a_child",
            format!(
                "`{}` is the value of a property: there is no room for what it holds",
                wrapper.ty
            ),
        ));
    };
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    let lists = studio
        .session
        .text(doc)
        .map(|(text, _)| studio_core::outline::more_than_a_place(text, wrapper.start))
        .map_err(error)?;
    // Only what places may go with it.
    let own: Vec<&str> = wrapper
        .properties
        .iter()
        .chain(&wrapper.functions)
        .map(String::as_str)
        .chain(wrapper.signals.iter().map(|s| s.name.as_str()))
        .chain(
            wrapper
                .bindings
                .iter()
                .map(|b| b.name.as_str())
                .filter(|n| !only_places(n)),
        )
        .chain(lists.iter().map(String::as_str))
        .chain(tree.iter().filter_map(|(_, n)| {
            let below = n.path.0.strip_prefix(wrapper.path.0.as_slice())?;
            (!matches!(below.first(), None | Some(Step::Child(_)))).then_some(n.ty.as_str())
        }))
        .collect();
    if let Some(name) = own.first() {
        return Err(refused(
            "holds_more",
            format!(
                "`{}` has its own `{name}`, which would go with it",
                label(&wrapper)
            ),
        ));
    }
    if let Some(id) = &wrapper.id
        && let Some(line) = read_elsewhere(studio, doc, &wrapper, id)?
    {
        return Err(refused(
            "referenced",
            format!("line {line} reads `{id}`, which would no longer be there"),
        ));
    }
    let children: Vec<(String, Node)> = tree
        .iter()
        .filter(|(_, n)| {
            parent_path(&n.path).as_ref() == Some(&wrapper.path)
                && matches!(n.path.0.last(), Some(Step::Child(_)))
        })
        .cloned()
        .collect();
    for (_, child) in &children {
        check_may_leave_parent(child)?;
    }
    let mut wanted = children.clone();
    wanted.push(outer.clone());
    let placed = placed_of(studio, doc, &wanted)?.unwrap_or_default();
    let types = parent_types(Some(&outer.1), placed.get(&outer.0));
    let free_outside = place::placer(&types).is_none();
    let free_inside = short(&wrapper.ty) == "Item";
    // Anchors or an expression put the wrapper where only a run knows.
    let anchored = wrapper
        .bindings
        .iter()
        .any(|b| b.name.starts_with("anchors."));
    let run_knows =
        |axis: &str| anchored || written(&wrapper, axis).is_some_and(|x| x.parse::<f64>().is_err());
    let mut ops = Vec::new();
    for (k, (child, node)) in children.iter().enumerate() {
        let child_address = Address::Node(child.clone());
        ops.push(Op::Move {
            node: child_address.clone(),
            parent: Address::Node(outer.0.clone()),
            index: Some(index + k),
        });
        if !free_outside {
            continue;
        }
        let shown = shown_in(&placed, child, placed.get(&outer.0));
        for (axis, live) in [("x", shown.map(|s| s.0)), ("y", shown.map(|s| s.1))] {
            if run_knows(axis) && live.is_none() {
                return Err(refused(
                    "needs_preview",
                    format!(
                        "where `{}` is depends on a run: open a preview of `{doc}` first",
                        label(&wrapper)
                    ),
                ));
            }
            let value = if run_knows(axis) {
                live.map(place::number)
            } else if free_inside {
                // An Item's own numbers are as true as the file is: they hold
                // with no preview, and with one that has yet to reload.
                let origin: Option<f64> = written(&wrapper, axis).unwrap_or("0").parse().ok();
                match origin {
                    Some(origin) if origin != 0.0 => Some(place::moved(
                        Some(written(node, axis).unwrap_or("0")),
                        origin,
                    )),
                    _ => None,
                }
            } else {
                live.map(place::number)
            };
            if let Some(value) = value {
                ops.push(Op::SetBinding {
                    node: child_address.clone(),
                    name: axis.to_owned(),
                    value,
                });
            }
        }
    }
    ops.push(Op::Remove {
        node: Address::Node(handle),
    });
    let (outcome, mut moved) = studio
        .session
        .edit_many(doc, generation, &ops, "unwrap", dry_run)
        .map_err(error)?;
    moved.truncate(children.len());
    Ok(ManyOut {
        edit: outcome.into(),
        nodes: moved,
        imported: Vec::new(),
    })
}

/// A binding that says where things are and nothing else, so that a wrapper
/// going loses nothing the numbers put on its children don't make up for.
fn only_places(name: &str) -> bool {
    name.starts_with("anchors.")
        || name.starts_with("Layout.")
        || matches!(
            name,
            "id" | "x"
                | "y"
                | "width"
                | "height"
                | "spacing"
                | "rowSpacing"
                | "columnSpacing"
                | "columns"
                | "rows"
                | "padding"
                | "leftPadding"
                | "rightPadding"
                | "topPadding"
                | "bottomPadding"
        )
}

/// The line of the first thing in `doc` that reads the object `id`, other than
/// the object saying so. A state's `target` is one, and a child's reading.
fn read_elsewhere(
    studio: &mut Studio,
    doc: &str,
    node: &Node,
    id: &str,
) -> Result<Option<usize>, SurfaceError> {
    let text = studio.session.text(doc).map_err(error)?.0;
    let own = node.bindings.iter().find(|b| b.name == "id");
    Ok(place::code_words(text)
        .into_iter()
        .filter(|w| !w.member && &text[w.start..w.end] == id)
        .find(|w| own.is_none_or(|b| w.start < b.start || w.end > b.end))
        .map(|w| text[..w.start].matches('\n').count() + 1))
}

/// The `anchors.*` of each of `nodes`, with what now disagrees taken away.
pub(crate) fn anchor(
    studio: &mut Studio,
    doc: &str,
    nodes: &[Address],
    pin: &Pin,
    generation: Option<u64>,
    dry_run: bool,
) -> Result<ManyOut, SurfaceError> {
    if pin.fill == Some(true) && pin.center_in == Some(true) {
        return Err(SurfaceError::invalid_argument(
            "`fill` and `centerIn` say different things",
        ));
    }
    if nodes.is_empty() {
        return Err(SurfaceError::invalid_argument("`anchor` needs `nodes`"));
    }
    let (tree, _) = studio.session.tree(doc).map_err(error)?;
    let at = |path| tree.iter().find(|(_, n)| n.path == path).cloned();
    let mut found = Vec::new();
    for address in nodes {
        let (handle, node) = studio.session.resolve(doc, address).map_err(error)?;
        let Some(parent) = parent_path(&node.path).and_then(at) else {
            return Err(refused(
                "root",
                "the document's root has nothing around it to pin to",
            ));
        };
        found.push((handle, node, parent));
    }
    let mut wanted: Vec<(String, Node)> = Vec::new();
    for (handle, node, parent) in &found {
        wanted.push((handle.clone(), node.clone()));
        wanted.push(parent.clone());
    }
    let placed = placed_of(studio, doc, &wanted)?.unwrap_or_default();
    let mut ops = Vec::new();
    for (handle, node, parent) in &found {
        let types = parent_types(Some(&parent.1), placed.get(&parent.0));
        if let Some(by) = place::placer(&types) {
            let instead = if by.ends_with("Layout") {
                "use `Layout.alignment` or `Layout.fillWidth`"
            } else {
                "use `spacing`, or `node_move` it out"
            };
            return Err(refused(
                "arranged",
                format!(
                    "`{}` is placed by its {by}, which anchors would fight: {instead}",
                    label(node)
                ),
            ));
        }
        if pin.target != "parent" {
            let sibling = tree.iter().any(|(_, n)| {
                n.id.as_deref() == Some(&pin.target)
                    && n.path != node.path
                    && parent_path(&n.path).as_ref() == Some(&parent.1.path)
            });
            if !sibling {
                return Err(refused(
                    "not_a_sibling",
                    format!(
                        "`{}` can be pinned to `parent` or a sibling, and `{}` is not one",
                        label(node),
                        pin.target
                    ),
                ));
            }
        }
        let now = placed.get(handle).map(|p| p.local);
        let now = Known {
            x: now.map(|r| r.x),
            y: now.map(|r| r.y),
            width: now.map(|r| r.width),
            height: now.map(|r| r.height),
        };
        let address = Address::Node(handle.clone());
        ops.extend(
            place::anchoring(node, pin, now)
                .into_iter()
                .map(|c| match c {
                    Change::Set(name, value) => Op::SetBinding {
                        node: address.clone(),
                        name,
                        value,
                    },
                    Change::Remove(name) => Op::RemoveBinding {
                        node: address.clone(),
                        name,
                    },
                }),
        );
    }
    let (outcome, _) = studio
        .session
        .edit_many(doc, generation, &ops, "anchor", dry_run)
        .map_err(error)?;
    Ok(ManyOut {
        edit: outcome.into(),
        nodes: found.into_iter().map(|(handle, ..)| handle).collect(),
        imported: Vec::new(),
    })
}
