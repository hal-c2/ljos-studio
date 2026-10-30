//! The tools, in wire order.

use std::sync::{Arc, Mutex};

use ljos_mcp_core::{
    CallCtx, Cap, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, denied, paginate, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use studio_core::{Address, Literal, Node, Op, Outcome};

use crate::{Studio, blocks, error, lock};
use serde_json::json;

/// What a document gets when `document_new` is given no text.
const NEW_DOCUMENT: &str = "import QtQuick\n\nItem {\n}\n";

/// Tree pages hold this many objects unless asked for fewer.
const TREE_PAGE: usize = 200;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectOpenArgs {
    /// A directory below the workspace, `.` for the workspace itself. Left
    /// out, the open project is described without reopening it.
    path: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct ProjectInfo {
    project: String,
    /// QML documents, relative to the project.
    documents: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TreeArgs {
    doc: String,
    /// From a previous page's `next`.
    cursor: Option<String>,
    /// At most this many objects (default 200).
    limit: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
struct TreeNode {
    /// The object's handle: stays true across edits.
    node: String,
    /// True of this generation only.
    path: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    depth: usize,
    line: usize,
    /// `name: value` as written, groups dotted (`font.bold`).
    bindings: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
struct Tree {
    doc: String,
    generation: u64,
    nodes: Vec<TreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TextArgs {
    doc: String,
    /// Byte offsets; the whole text when left out.
    start: Option<usize>,
    end: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
struct Text {
    doc: String,
    generation: u64,
    start: usize,
    end: usize,
    text: String,
    /// Whether the whole document parses; structural edits wait until it does.
    parses: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewArgs {
    /// A new `.qml` path relative to the project.
    doc: String,
    /// Its text; an empty `Item` importing QtQuick when left out.
    text: Option<String>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditTextArgs {
    doc: String,
    /// The generation the offsets were read at.
    generation: u64,
    /// Byte range to replace; the whole text when both are left out.
    start: Option<usize>,
    end: Option<usize>,
    text: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Serialize, JsonSchema)]
struct ChangeOut {
    /// Byte range of the text before the edit.
    start: usize,
    end: usize,
    text: String,
}

#[derive(Serialize, JsonSchema)]
struct EditOut {
    doc: String,
    /// The document's generation now; pass it with the next edit.
    generation: u64,
    changes: Vec<ChangeOut>,
    /// The history entry; absent for a dry run or an unchanged document.
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<u64>,
    /// The handle of the object put or moved.
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<String>,
    /// The text the edit would give, for a dry run.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// The document already said this: nothing was written and no history
    /// entry made.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    unchanged: bool,
}

impl From<Outcome> for EditOut {
    fn from(o: Outcome) -> Self {
        Self {
            doc: o.doc,
            generation: o.generation,
            changes: o
                .changes
                .into_iter()
                .map(|c| ChangeOut {
                    start: c.range.start,
                    end: c.range.end,
                    text: c.replacement,
                })
                .collect(),
            history: o.history,
            node: o.node,
            unchanged: o.unchanged,
            text: o.text,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FindArgs {
    /// One document; every one that parses when left out.
    doc: Option<String>,
    id: Option<String>,
    #[serde(rename = "type")]
    ty: Option<String>,
    /// Has a binding of this name (`font.bold`).
    binding: Option<String>,
    /// Some binding's value contains this text.
    text: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct Found {
    doc: String,
    node: String,
    path: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct FindOut {
    nodes: Vec<Found>,
    /// Documents left out because they do not parse.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    broken: Vec<String>,
    /// Documents left out because they could not be read (not UTF-8, gone
    /// since they were listed); `document_text` on one says why.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unreadable: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SelectArgs {
    doc: String,
    /// Empty clears the selection.
    nodes: Vec<Address>,
}

#[derive(Serialize, JsonSchema)]
struct Selection {
    selection: Vec<Found>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NodeArgs {
    doc: String,
    node: Address,
}

#[derive(Serialize, JsonSchema)]
struct BindingOut {
    name: String,
    value: String,
    start: usize,
    end: usize,
    /// A `property` this object declares, not a setting of an inherited one.
    declared: bool,
}

#[derive(Serialize, JsonSchema)]
struct Inspected {
    doc: String,
    generation: u64,
    node: String,
    path: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    start: usize,
    end: usize,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    bindings: Vec<BindingOut>,
    children: Vec<Found>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InsertArgs {
    doc: String,
    /// The generation read; refused if the document moved on.
    generation: Option<u64>,
    parent: Address,
    /// Child position; the end when left out.
    index: Option<usize>,
    /// One object in QML (`Rectangle { color: "red" }`). With `block`, it may
    /// be left out for the bare block, or set its inputs.
    text: Option<String>,
    /// A functional block from `blocks_list`, `Module.Name`: its module is
    /// imported and `text` must be of its type.
    block: Option<String>,
    /// A module the document must import for it (`Shop.Blocks`).
    import: Option<String>,
    /// Give the object an `id` if it has none (default true).
    id: Option<bool>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MoveArgs {
    doc: String,
    generation: Option<u64>,
    node: Address,
    parent: Address,
    /// Child position in the new parent, counted without the moved object;
    /// the end when left out.
    index: Option<usize>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RemoveArgs {
    doc: String,
    generation: Option<u64>,
    node: Address,
    #[serde(default)]
    dry_run: bool,
}

/// A literal a property is set to.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum LiteralArg {
    Bool(bool),
    Number(serde_json::Number),
    String(String),
}

/// A value that must be given, `null` included. Serde reads a missing
/// `Option` field as `None`, so a `value` left out by mistake would remove
/// a binding; this type is not an `Option`, so a missing one is refused,
/// and its schema is `Option<T>`'s: required, and `null` allowed.
struct Present<T>(Option<T>);

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Present<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Serde fills a missing field by asking for an `Option`, which says
        // `None`; asked for any value, it says the field is missing.
        let value = serde_json::Value::deserialize(d)?;
        serde_json::from_value(value)
            .map(Present)
            .map_err(serde::de::Error::custom)
    }
}

impl<T: JsonSchema> JsonSchema for Present<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        <Option<T>>::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <Option<T>>::json_schema(generator)
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PropertyArgs {
    doc: String,
    generation: Option<u64>,
    node: Address,
    /// `width`, `font.bold`; written grouped or dotted as the file already does.
    name: String,
    /// `null` removes the binding. Required: left out, nothing is removed.
    value: Present<LiteralArg>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BindingArgs {
    doc: String,
    generation: Option<u64>,
    node: Address,
    /// A property (`width`) or a handler (`onClicked`).
    name: String,
    /// QML after the `:` (`parent.width / 2`, `{ count++ }`); `null` removes it.
    /// Required: left out, nothing is removed.
    value: Present<String>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum HistoryAction {
    List,
    Undo,
    Redo,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HistoryArgs {
    action: HistoryAction,
}

#[derive(Serialize, JsonSchema)]
struct EntryOut {
    id: u64,
    doc: String,
    label: String,
    removed: usize,
    added: usize,
}

impl From<studio_core::EntrySummary> for EntryOut {
    fn from(e: studio_core::EntrySummary) -> Self {
        Self {
            id: e.id,
            doc: e.doc,
            label: e.label,
            removed: e.removed,
            added: e.added,
        }
    }
}

#[derive(Serialize, JsonSchema)]
struct HistoryOut {
    /// The entry undone or redone.
    #[serde(skip_serializing_if = "Option::is_none")]
    entry: Option<EntryOut>,
    /// Its document's generation after it, when the document still exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<u64>,
    /// Oldest first.
    done: Vec<EntryOut>,
    /// Next to redo first.
    undone: Vec<EntryOut>,
}

fn found(doc: &str, handle: String, node: &Node) -> Found {
    Found {
        doc: doc.to_owned(),
        node: handle,
        path: node.path.to_string(),
        ty: node.ty.clone(),
        id: node.id.clone(),
    }
}

fn info(studio: &Studio) -> ProjectInfo {
    ProjectInfo {
        project: studio.project().to_owned(),
        documents: studio.session.documents(),
    }
}

const MUTATE: ToolAnnotations = ToolAnnotations {
    read_only: false,
    destructive: false,
    idempotent: false,
    open_world: false,
};

const DESTRUCTIVE: ToolAnnotations = ToolAnnotations {
    destructive: true,
    ..MUTATE
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BlocksArgs {
    /// Only this module's blocks (`App.Media`).
    module: Option<String>,
    /// From a previous page's `next`.
    cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct BlocksOut {
    /// By module, then name: `module`, `name`, `version`, `origin`,
    /// `category`?, `summary`?, and `state`, `inputs`, `actions`, `events`,
    /// `slots` as `ljos_blocks` writes them.
    blocks: Vec<serde_json::Value>,
    /// Declarations that could not be read: `file`, `line`?, `message`.
    errors: Vec<serde_json::Value>,
    next: Option<String>,
}

const BLOCKS_PAGE: usize = 50;

/// What `node_insert` inserts and imports: its own `text` and `import`, or
/// a block's.
fn insert_text(
    studio: &Mutex<Studio>,
    a: &InsertArgs,
) -> Result<(String, Option<String>), SurfaceError> {
    let Some(qualified) = &a.block else {
        let text = a.text.clone().ok_or_else(|| {
            SurfaceError::invalid_argument("give the object's `text`, or a `block`")
        })?;
        return Ok((text, a.import.clone()));
    };
    if a.import.is_some() {
        return Err(SurfaceError::invalid_argument(
            "a block brings its own import; leave `import` out",
        ));
    }
    let registry = blocks::registry(lock(studio).session.store());
    let Some(block) = blocks::find(&registry, qualified) else {
        return Err(SurfaceError::not_found(format!(
            "no block `{qualified}`; `blocks_list` lists them"
        ))
        .with_details(json!({ "kind": "no_block" })));
    };
    let text = match &a.text {
        None => format!("{} {{}}", block.name),
        Some(text) => {
            let ty = text
                .trim_start()
                .split(|c: char| c == '{' || c.is_whitespace())
                .next();
            if ty != Some(block.name.as_str()) {
                return Err(SurfaceError::invalid_argument(format!(
                    "`text` must be a `{}` to insert the block `{qualified}`",
                    block.name
                )));
            }
            text.clone()
        }
    };
    Ok((text, Some(block.module.clone())))
}

fn edit(
    studio: &Mutex<Studio>,
    doc: &str,
    generation: Option<u64>,
    op: &Op,
    dry_run: bool,
) -> Result<EditOut, SurfaceError> {
    let mut studio = lock(studio);
    studio
        .session
        .edit(doc, generation, op, dry_run)
        .map(EditOut::from)
        .map_err(error)
}

pub(crate) fn registry(studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let s = || studio.clone();
    ToolRegistry::new()
        .with(
            ToolDef::new::<ProjectOpenArgs>(
                "project_open",
                "Open a project directory below the workspace, or describe the open one. Opening \
                 starts a new session: earlier handles and history end.",
            )
            .output::<ProjectInfo>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: ProjectOpenArgs| {
                    let mut studio = lock(&studio);
                    if let Some(path) = a.path {
                        studio.switch(&path)?;
                    }
                    Ok(info(&studio))
                }
            }),
        )
        .with(
            ToolDef::new::<TreeArgs>(
                "document_tree",
                "A document's objects in order: handle, path, type, id, bindings. Grouped \
                 properties are bindings (`font.bold`), not objects. Paginated by `cursor`.",
            )
            .output::<Tree>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: TreeArgs| {
                    let mut studio = lock(&studio);
                    let (tree, generation) = studio.session.tree(&a.doc).map_err(error)?;
                    let nodes = tree
                        .into_iter()
                        .map(|(node, n)| TreeNode {
                            node,
                            path: n.path.to_string(),
                            ty: n.ty,
                            id: n.id,
                            depth: n.depth,
                            line: n.line,
                            bindings: n
                                .bindings
                                .into_iter()
                                .map(|b| format!("{}: {}", b.name, b.value))
                                .collect(),
                        })
                        .collect();
                    let limit = a.limit.unwrap_or(TREE_PAGE).clamp(1, TREE_PAGE);
                    let (nodes, next) = paginate(nodes, a.cursor.as_deref(), limit)?;
                    Ok(Tree {
                        doc: a.doc,
                        generation,
                        nodes,
                        next,
                    })
                }
            }),
        )
        .with(
            ToolDef::new::<TextArgs>(
                "document_text",
                "A document's text or a byte range of it, with its generation and whether it parses.",
            )
            .output::<Text>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: TextArgs| {
                    let mut studio = lock(&studio);
                    let parses = studio.session.tree(&a.doc).is_ok();
                    let (text, generation) = studio.session.text(&a.doc).map_err(error)?;
                    let start = a.start.unwrap_or(0);
                    let end = a.end.unwrap_or(text.len());
                    let slice = text.get(start..end).ok_or_else(|| {
                        SurfaceError::invalid_argument(format!("{start}..{end} is not a range of the text"))
                    })?;
                    Ok(Text {
                        doc: a.doc,
                        generation,
                        start,
                        end,
                        text: slice.to_owned(),
                        parses,
                    })
                }
            }),
        )
        .with(
            ToolDef::new::<NewArgs>(
                "document_new",
                "Create a QML document. Refused if it exists or does not parse. Undo removes it.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(MUTATE),
            typed({
                let studio = s();
                move |_, a: NewArgs| {
                    let mut studio = lock(&studio);
                    let text = a.text.as_deref().unwrap_or(NEW_DOCUMENT);
                    studio
                        .session
                        .create(&a.doc, text, a.dry_run)
                        .map(EditOut::from)
                        .map_err(error)
                }
            }),
        )
        .with(
            ToolDef::new::<EditTextArgs>(
                "document_edit",
                "Replace a byte range of a document's text, as the code view does. The result may \
                 not parse; structural edits then wait and handles hold. Needs the generation read.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |_, a: EditTextArgs| {
                    let range = match (a.start, a.end) {
                        (None, None) => None,
                        (Some(start), Some(end)) => Some(start..end),
                        _ => {
                            return Err(SurfaceError::invalid_argument(
                                "give both `start` and `end`, or neither",
                            ));
                        }
                    };
                    let mut studio = lock(&studio);
                    studio
                        .session
                        .replace_text(&a.doc, Some(a.generation), range, &a.text, a.dry_run)
                        .map(EditOut::from)
                        .map_err(error)
                }
            }),
        )
        .with(
            ToolDef::new::<FindArgs>(
                "node_find",
                "Find objects by id, type, binding name or text in a binding's value, in one \
                 document or all (naming those it could not parse or read). Returns handles.",
            )
            .output::<FindOut>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: FindArgs| {
                    let mut studio = lock(&studio);
                    let docs = match &a.doc {
                        Some(doc) => vec![doc.clone()],
                        None => studio.session.documents(),
                    };
                    let mut out = FindOut {
                        nodes: Vec::new(),
                        broken: Vec::new(),
                        unreadable: Vec::new(),
                    };
                    for doc in docs {
                        let tree = match studio.session.tree(&doc) {
                            Ok((tree, _)) => tree,
                            Err(studio_core::Error::Broken { .. }) if a.doc.is_none() => {
                                out.broken.push(doc);
                                continue;
                            }
                            // One file a search cannot read does not hide
                            // what the others hold.
                            Err(studio_core::Error::Store(_)) if a.doc.is_none() => {
                                out.unreadable.push(doc);
                                continue;
                            }
                            Err(e) => return Err(error(e)),
                        };
                        for (handle, node) in tree {
                            let keep = a.id.as_ref().is_none_or(|id| node.id.as_ref() == Some(id))
                                && a.ty.as_ref().is_none_or(|ty| node.ty == *ty)
                                && a.binding
                                    .as_ref()
                                    .is_none_or(|name| node.bindings.iter().any(|b| b.name == *name))
                                && a.text.as_ref().is_none_or(|text| {
                                    node.bindings.iter().any(|b| b.value.contains(text.as_str()))
                                });
                            if keep {
                                out.nodes.push(found(&doc, handle, &node));
                            }
                        }
                    }
                    Ok(out)
                }
            }),
        )
        .with(
            ToolDef::new::<SelectArgs>(
                "node_select",
                "Set Studio's selection, shown on the canvas and in the inspector. Empty clears it.",
            )
            .output::<Selection>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: SelectArgs| {
                    let mut studio = lock(&studio);
                    studio.session.select(&a.doc, &a.nodes).map_err(error)?;
                    let mut selection = Vec::new();
                    for (doc, handle) in studio.session.selection() {
                        let (handle, node) = studio
                            .session
                            .resolve(&doc, &Address::Node(handle))
                            .map_err(error)?;
                        selection.push(found(&doc, handle, &node));
                    }
                    Ok(Selection { selection })
                }
            }),
        )
        .with(
            ToolDef::new::<NodeArgs>(
                "node_inspect",
                "An object's bindings with their byte ranges, its parent and its children.",
            )
            .output::<Inspected>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: NodeArgs| {
                    let mut studio = lock(&studio);
                    let (handle, node) = studio.session.resolve(&a.doc, &a.node).map_err(error)?;
                    let (tree, generation) = studio.session.tree(&a.doc).map_err(error)?;
                    let parent_path = (!node.path.0.is_empty())
                        .then(|| studio_core::NodePath(node.path.0[..node.path.0.len() - 1].to_vec()));
                    let parent = parent_path.and_then(|p| {
                        tree.iter().find(|(_, n)| n.path == p).map(|(h, _)| h.clone())
                    });
                    let children = tree
                        .iter()
                        .filter(|(_, n)| {
                            n.path.0.len() == node.path.0.len() + 1 && n.path.0.starts_with(&node.path.0)
                        })
                        .map(|(h, n)| found(&a.doc, h.clone(), n))
                        .collect();
                    Ok(Inspected {
                        doc: a.doc,
                        generation,
                        node: handle,
                        path: node.path.to_string(),
                        ty: node.ty,
                        id: node.id,
                        start: node.start,
                        end: node.end,
                        line: node.line,
                        parent,
                        bindings: node
                            .bindings
                            .into_iter()
                            .map(|b| BindingOut {
                                name: b.name,
                                value: b.value,
                                start: b.start,
                                end: b.end,
                                declared: b.declared,
                            })
                            .collect(),
                        children,
                    })
                }
            }),
        )
        .with(
            ToolDef::new::<BlocksArgs>(
                "blocks_list",
                "List the project's functional blocks (`# block` qmldir comments, `#[qml(block)]` \
                 types in ljostypes.json) with their contracts: state, inputs, actions, events and \
                 slots. `node_insert` places one by `Module.Name`. Unreadable ones are in `errors`.",
            )
            .output::<BlocksOut>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: BlocksArgs| {
                    let registry = blocks::registry(lock(&studio).session.store());
                    let registry = match &a.module {
                        Some(module) => registry.of_module(module),
                        None => registry,
                    };
                    let value = registry.to_value();
                    let list = |key: &str| value[key].as_array().cloned().unwrap_or_default();
                    let (blocks, next) = paginate(list("blocks"), a.cursor.as_deref(), BLOCKS_PAGE)?;
                    Ok(BlocksOut {
                        blocks,
                        errors: list("errors"),
                        next,
                    })
                }
            }),
        )
        .with(
            ToolDef::new::<InsertArgs>(
                "node_insert",
                "Insert one object written in QML, or a `block` from `blocks_list`, as a child at an \
                 index. It gets an id unless it has one; `import` (a block's module) is added once. \
                 Returns the new object's handle.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(MUTATE),
            typed({
                let studio = s();
                move |_, a: InsertArgs| {
                    let (text, import) = insert_text(&studio, &a)?;
                    let op = Op::Insert {
                        parent: a.parent,
                        index: a.index,
                        text,
                        add_id: a.id.unwrap_or(true),
                        import,
                    };
                    edit(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<MoveArgs>(
                "node_move",
                "Move an object to a parent and child index, with its comments: reorder or reparent.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(MUTATE),
            typed({
                let studio = s();
                move |_, a: MoveArgs| {
                    let op = Op::Move {
                        node: a.node,
                        parent: a.parent,
                        index: a.index,
                    };
                    edit(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<RemoveArgs>(
                "node_remove",
                "Remove an object with its comments. Its handle then answers `stale_ref` until undo.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |_, a: RemoveArgs| {
                    edit(&studio, &a.doc, a.generation, &Op::Remove { node: a.node }, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<PropertyArgs>(
                "property_set",
                "Set a property to a literal (bool, number, string) or remove it with null. Grouped \
                 and dotted forms are one property; a new one follows the file's style.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(MUTATE),
            typed({
                let studio = s();
                move |_, a: PropertyArgs| {
                    let op = match a.value.0 {
                        None => Op::RemoveBinding {
                            node: a.node,
                            name: a.name,
                        },
                        Some(value) => {
                            let literal = match value {
                                LiteralArg::Bool(b) => Literal::Bool(b),
                                LiteralArg::Number(n) => Literal::Number(n.to_string()),
                                LiteralArg::String(s) => Literal::String(s),
                            };
                            Op::SetBinding {
                                node: a.node,
                                name: a.name,
                                value: literal.to_qml(),
                            }
                        }
                    };
                    edit(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<BindingArgs>(
                "binding_set",
                "Set a binding expression or a signal handler, or remove it with null. Refused \
                 with `would_break` if the file would not parse.",
            )
            .output::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(MUTATE),
            typed({
                let studio = s();
                move |_, a: BindingArgs| {
                    let op = match a.value.0 {
                        None => Op::RemoveBinding {
                            node: a.node,
                            name: a.name,
                        },
                        Some(value) => Op::SetBinding {
                            node: a.node,
                            name: a.name,
                            value,
                        },
                    };
                    edit(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<HistoryArgs>(
                "history",
                "List edits (`inspect`), or undo or redo the last one (`mutate`). Undo gives back \
                 the document's bytes.",
            )
            .output::<HistoryOut>()
            .requires(Cap::Inspect)
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |ctx: &CallCtx, a: HistoryArgs| {
                    // Listing only reads; the registry checked `inspect`, the
                    // rest is checked here, refused the way the registry does.
                    if !matches!(a.action, HistoryAction::List) && !ctx.caps.contains(Cap::Mutate) {
                        return Err(denied("history", Cap::Mutate, ctx));
                    }
                    let mut studio = lock(&studio);
                    let entry = match a.action {
                        HistoryAction::List => None,
                        HistoryAction::Undo => Some(studio.session.undo().map_err(error)?),
                        HistoryAction::Redo => Some(studio.session.redo().map_err(error)?),
                    };
                    let generation = match &entry {
                        Some(e) if studio.session.store().exists(&e.doc) => {
                            Some(studio.session.generation(&e.doc).map_err(error)?)
                        }
                        _ => None,
                    };
                    let (done, undone) = studio.session.history();
                    Ok(HistoryOut {
                        entry: entry.map(EntryOut::from),
                        generation,
                        done: done.into_iter().map(EntryOut::from).collect(),
                        undone: undone.into_iter().map(EntryOut::from).collect(),
                    })
                }
            }),
        )
}
