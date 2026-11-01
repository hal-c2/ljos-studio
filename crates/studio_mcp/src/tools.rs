//! The tools, in wire order.

use std::sync::{Arc, Mutex};

use crate::assets::{self, AssetAction, AssetInfo};
use crate::schema::Answers;
use ljos_mcp_core::{
    CallCtx, Cap, ErrorCode, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, denied,
    paginate, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use studio_core::{
    Address, Goal, Interaction, Literal, Mode, Model, Node, Op, Outcome, StateEdit, StateInfo,
    TransitionInfo, preview::Target,
};

use crate::{Studio, blocks, error, lock, store_error};
use serde_json::json;

/// What a document gets when `document_new` is given no text.
const NEW_DOCUMENT: &str = "import QtQuick\n\nItem {\n}\n";

/// Tree pages hold this many objects unless asked for fewer.
const TREE_PAGE: usize = 200;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectOpenArgs {
    /// A directory below the workspace, relative or absolute (`.` is the workspace). Left out, describes the open project.
    path: Option<String>,
    /// On the project's `assets/`: `list` it, `import` a picture or font (`fs`), `remove` one no document uses (`fs`).
    asset: Option<AssetAction>,
    /// `import`: a file below the workspace.
    file: Option<String>,
    /// `import`: the file's bytes in base64, for a client with no file here; needs `name`.
    data: Option<String>,
    /// `import`: the name to give it. `remove`: the asset's name.
    name: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct ProjectInfo {
    project: String,
    /// QML documents, relative to the project.
    documents: Vec<String>,
    /// What `list` found.
    #[serde(skip_serializing_if = "Option::is_none")]
    assets: Option<Vec<AssetInfo>>,
    /// What `import` wrote.
    #[serde(skip_serializing_if = "Option::is_none")]
    asset: Option<AssetInfo>,
    /// The url `remove` took out.
    #[serde(skip_serializing_if = "Option::is_none")]
    removed: Option<String>,
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
    /// A project JSON file of records: a `JsonListModel` of it, or with `model: "list"` a `ListModel` copy. Not with `text`.
    data: Option<String>,
    /// How `data` is kept: `json` (the default), or `list`.
    #[serde(default)]
    model: DataModel,
    #[serde(default)]
    dry_run: bool,
}

/// How a document keeps a JSON file's records.
#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum DataModel {
    /// A `JsonListModel` whose `source` is the file: the file stays the
    /// data.
    #[default]
    Json,
    /// A `ListModel` copy of the records, edited as QML from then on.
    List,
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
    /// A person's typing: joins the last in one undo step until a pause of a second or a new word.
    #[serde(default)]
    typing: bool,
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
pub(crate) struct EditOut {
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
    // Always serialised, `false` on a write: the output schema requires it.
    unchanged: bool,
    /// The `generation` the edit was made against, when the document had
    /// moved on since and the edit was applied past what others wrote.
    #[serde(skip_serializing_if = "Option::is_none")]
    rebased_from: Option<u64>,
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
            rebased_from: o.rebased,
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
    // This and `unreadable` are serialised when empty too: the output
    // schema requires both.
    broken: Vec<String>,
    /// Documents left out because they could not be read (not UTF-8, gone
    /// since they were listed); `document_text` on one says why.
    unreadable: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SelectArgs {
    doc: String,
    /// Empty or null clears the selection.
    #[serde(default, deserialize_with = "list_or_null")]
    nodes: Vec<Address>,
    /// Instead of `nodes`: a point in the open preview, in logical pixels; selects what is declared where the topmost object is. Nothing there clears it.
    at: Option<Point>,
    /// Toggle these in the selection instead of replacing it.
    #[serde(default)]
    add: bool,
    /// Also hold the selected objects and what they hold (`mutate`): others' edits of them are `busy` until the caller is idle five seconds (five minutes at most). Selecting nothing lets go.
    #[serde(default)]
    hold: bool,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Serialize, JsonSchema)]
struct Selection {
    selection: Vec<Found>,
    /// The handles of `doc` the caller holds.
    held: Vec<String>,
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
    /// Its `State`s in order, each with the values it changes as written.
    states: Vec<StateInfo>,
    transitions: Vec<TransitionInfo>,
    /// The flows written in it and in everything it holds, in order.
    interactions: Vec<Interaction>,
    /// The data it sees: its own as a `ListModel`, a list's model's, or
    /// in a list's delegate the list's, whose roles bind as `model.role`.
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<Model>,
    /// Every signal it has, which a flow's `signal` may name: what it
    /// declares, then its type's own and inherited, each with its
    /// arguments. Absent for a type described nowhere Studio looks (no
    /// import, project file, type dump or Ljós type has it).
    #[serde(skip_serializing_if = "Option::is_none")]
    signals: Option<Vec<crate::signals::Signal>>,
    /// What its type takes that the project says: the properties its files
    /// and type dumps declare, written here or not.
    takes: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InsertArgs {
    doc: String,
    /// The generation read; a `path` needs it (handles and ids are moved past others' edits).
    generation: Option<u64>,
    parent: Address,
    /// Child position; the end when left out.
    index: Option<usize>,
    /// One object in QML, or one member (`function f() {}`, `signal s()`, `property int n`, `onClicked: f()`). With `block`: left out for the bare block, or its inputs.
    text: Option<String>,
    /// A block from `blocks_list`, `Module.Name`; imported, and `text` must be of its type.
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
    /// The generation read; a `path` needs it (handles and ids are moved past others' edits).
    generation: Option<u64>,
    node: Address,
    parent: Address,
    /// Index in the new parent, without the moved object; the end if left out.
    index: Option<usize>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RemoveArgs {
    doc: String,
    /// The generation read; a `path` needs it (handles and ids are moved past others' edits).
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
    /// The generation read; a `path` needs it (handles and ids are moved past others' edits).
    generation: Option<u64>,
    node: Address,
    /// `width`, `font.bold`; written grouped or dotted as the file already does.
    name: String,
    /// `null` removes the binding. Required.
    value: Present<LiteralArg>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BindingArgs {
    doc: String,
    /// The generation read; a `path` needs it (handles and ids are moved past others' edits).
    generation: Option<u64>,
    node: Address,
    /// A property (`width`) or a handler (`onClicked`).
    name: String,
    /// QML after the `:` (`parent.width / 2`, `{ count++ }`); `null` removes it. Required.
    value: Present<String>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum StateAction {
    Add,
    Rename,
    Remove,
    /// Set or remove one value the state changes.
    Change,
    /// Set or remove the state's `when`.
    When,
    AddTransition,
    RemoveTransition,
    /// A flow on `trigger`: to `state` or, by `navigator`, to `open`.
    Interaction,
    RemoveInteraction,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StateEditArgs {
    doc: String,
    generation: Option<u64>,
    /// The object whose states these are; the root when left out.
    node: Option<Address>,
    action: StateAction,
    /// The state's name; all but the transition actions need it.
    state: Option<String>,
    /// `rename`: the new name.
    new_name: Option<String>,
    /// `add`, `when`: the condition, QML after `when:`; left out of `when`, removes it.
    when: Option<String>,
    /// `add`, `when`: a preview target whose width bound is the `when`.
    breakpoint: Option<Target>,
    /// `change`: the object, which must have an id.
    target: Option<Address>,
    /// `change`: its property (`text`, `anchors.left`).
    name: Option<String>,
    /// `change`: a literal; with neither this nor `binding`, the value is removed.
    value: Option<LiteralArg>,
    /// `change`: an expression (`root.width / 2`).
    binding: Option<String>,
    /// Transitions: the states between; `*` (any) if left out.
    from: Option<String>,
    to: Option<String>,
    /// `add_transition`: the animations it runs, in QML.
    animations: Option<String>,
    /// Interactions: the object tapped.
    trigger: Option<Address>,
    /// Interactions: its signal (`clicked`); a `TapHandler`'s tap if left out.
    signal: Option<String>,
    /// `interaction`: a document beside this one to open, not with `state`.
    open: Option<String>,
    /// `interaction`: the `Loader` or `StackView` that opens it.
    navigator: Option<Address>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ContributeArgs {
    doc: String,
    generation: Option<u64>,
    /// The slot, as `blocks_list` lists it (`controls`).
    slot: Option<String>,
    /// Where the `Contribution` goes; the root when left out.
    parent: Option<Address>,
    /// How it joins what the slot holds; the runtime's default when left out.
    mode: Option<Mode>,
    /// Its place among the slot's contributions.
    order: Option<i64>,
    /// What it shows: one object in QML.
    text: Option<String>,
    /// Or a block from `blocks_list`, `Module.Name`, shown bare.
    block: Option<String>,
    /// Take this `Contribution` out instead.
    remove: Option<Address>,
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
    /// Undo or redo the newest edit whoever made it, not the caller's own.
    #[serde(default)]
    all: bool,
}

#[derive(Serialize, JsonSchema)]
struct EntryOut {
    id: u64,
    doc: String,
    label: String,
    /// Who made it: an MCP client's name, `agent` for one that gave none,
    /// `person` for the window, `outside Studio` for a change found in the
    /// file.
    author: String,
    removed: usize,
    added: usize,
}

impl From<studio_core::EntrySummary> for EntryOut {
    fn from(e: studio_core::EntrySummary) -> Self {
        Self {
            id: e.id,
            doc: e.doc,
            label: e.label,
            author: e.author,
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

#[derive(Deserialize, JsonSchema, Clone, Copy)]
enum Variant {
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
    #[serde(rename = "lightHigh")]
    LightHigh,
    #[serde(rename = "darkHigh")]
    DarkHigh,
}

impl Variant {
    fn name(self) -> &'static str {
        match self {
            Variant::Light => "light",
            Variant::Dark => "dark",
            Variant::LightHigh => "lightHigh",
            Variant::DarkHigh => "darkHigh",
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ThemeArgs {
    /// Whose theme: the nearest `theme.json` up from it; left out, the preview's, else the project's.
    doc: Option<String>,
    /// `Colors.accent`; a singleton (`Spacing`) reads its roles. Every role
    /// when left out.
    token: Option<String>,
    /// What the token becomes; `null` gives it back to the module. Left
    /// out, the tokens are read.
    #[serde(default, deserialize_with = "present")]
    #[schemars(with = "Option<serde_json::Value>")]
    value: Option<Option<serde_json::Value>>,
    /// One variant of a colour; a colour string sets light and dark.
    variant: Option<Variant>,
    /// The theme file's generation the edit was read at; 0 while it does
    /// not exist.
    generation: Option<u64>,
    #[serde(default)]
    dry_run: bool,
}

/// A list that may be `null`: an agent that has nothing to give often says
/// so with `null`, and means what an empty list means.
fn list_or_null<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<Vec<T>>::deserialize(d).map(Option::unwrap_or_default)
}

/// A field that is there, `null` or not: `Some(None)` for `null`.
fn present<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<serde_json::Value>>, D::Error> {
    Option::<serde_json::Value>::deserialize(d).map(Some)
}

#[derive(Serialize, JsonSchema)]
struct TokenOut {
    token: String,
    /// `color`, `int`, `real` or `curve`.
    kind: &'static str,
    /// The module's value; a colour's light, dark, lightHigh and darkHigh.
    module: serde_json::Value,
    /// The theme file's, absent when it sets none.
    #[serde(skip_serializing_if = "Option::is_none")]
    theme: Option<serde_json::Value>,
}

#[derive(Serialize, JsonSchema)]
struct ThemeOut {
    /// The theme file.
    file: String,
    /// Its generation; 0 while it does not exist.
    generation: u64,
    /// Read: whether the file exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    exists: Option<bool>,
    /// Read: why Ljós would not load the file; its values are left out.
    #[serde(skip_serializing_if = "Option::is_none")]
    problem: Option<String>,
    /// Read: the tokens asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    tokens: Option<Vec<TokenOut>>,
    /// Written: the edit, as other edits answer it.
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changes: Option<Vec<ChangeOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Written: whether the file already said this, so nothing was written
    /// and no history entry made. In every write's answer; a read has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    unchanged: Option<bool>,
}

/// The theme file of `doc`: `doc` itself when it is one; else the one Ljós
/// reads for an entry there (`ljos_design::theme::find`), the nearest
/// `theme.json` from its directory up to the project's; else the project's
/// own, where Ljós looks and watches when there is none, so a new one is
/// made there. A `doc` outside the project is answered as itself, a path the
/// store refuses: the file beside it can be the project's own when it sits
/// directly under `/`, or is a link at the project's top.
pub(crate) fn theme_file(store: &dyn studio_core::Store, doc: &str) -> String {
    let file = studio_core::theme::FILE;
    if doc.is_empty() {
        return file.to_owned();
    }
    if doc == file || doc.ends_with(&format!("/{file}")) {
        return doc.to_owned();
    }
    let beside = |dir: &str| match dir {
        "" => file.to_owned(),
        dir => format!("{dir}/{file}"),
    };
    let mut dir = doc.rsplit_once('/').map_or("", |(dir, _)| dir);
    // `MemStore` confines nothing, so the path is judged here as well as by
    // the store, which alone sees where a link leads.
    let plain = std::path::Path::new(doc)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !plain || store.resolved(doc).is_err() {
        return doc.to_owned();
    }
    loop {
        if store.exists(&beside(dir)) {
            return beside(dir);
        }
        match dir.rsplit_once('/') {
            Some((up, _)) => dir = up,
            None if !dir.is_empty() => dir = "",
            None => break,
        }
    }
    file.to_owned()
}

/// The theme file Ljós reads for `doc` when it runs it: it starts from the
/// real file, so a link in the project has the theme of what it leads to. An
/// export keeps the path as written ([`theme_file`]): a bundle lists a linked
/// file as a real one under the link's name.
pub(crate) fn running_theme_file(store: &dyn studio_core::Store, doc: &str) -> String {
    // A theme file is itself under its own name, wherever a link takes it.
    if doc.rsplit('/').next() == Some(studio_core::theme::FILE) {
        return theme_file(store, doc);
    }
    match store.resolved(doc) {
        Ok(real) => theme_file(store, &real),
        Err(_) => theme_file(store, doc),
    }
}

fn theme_set(studio: &Mutex<Studio>, a: ThemeArgs) -> Result<ThemeOut, SurfaceError> {
    let mut studio = lock(studio);
    let doc = a.doc.or_else(|| {
        crate::preview::lock(&studio.previews)
            .open
            .as_ref()
            .map(|o| o.doc.clone())
    });
    let file = running_theme_file(studio.session.store(), doc.as_deref().unwrap_or(""));
    let Some(value) = a.value else {
        let read = studio
            .session
            .tokens(&file, a.token.as_deref())
            .map_err(error)?;
        return Ok(ThemeOut {
            file: read.file,
            generation: read.generation,
            exists: Some(read.exists),
            problem: read.problem,
            tokens: Some(
                read.tokens
                    .into_iter()
                    .map(|t| TokenOut {
                        token: t.token,
                        kind: t.kind,
                        module: t.module,
                        theme: t.theme,
                    })
                    .collect(),
            ),
            doc: None,
            changes: None,
            history: None,
            text: None,
            unchanged: None,
        });
    };
    let token = a.token.ok_or_else(|| {
        SurfaceError::new(
            ErrorCode::InvalidArgument,
            "a `value` needs the `token` it is for",
        )
    })?;
    let outcome = studio
        .session
        .set_token(
            &file,
            a.generation,
            &token,
            a.variant.map(Variant::name),
            value.as_ref(),
            a.dry_run,
        )
        .map_err(error)?;
    let edit = EditOut::from(outcome);
    Ok(ThemeOut {
        file: edit.doc.clone(),
        generation: edit.generation,
        exists: None,
        problem: None,
        tokens: None,
        doc: Some(edit.doc),
        changes: Some(edit.changes),
        history: edit.history,
        text: edit.text,
        unchanged: Some(edit.unchanged),
    })
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
        assets: None,
        asset: None,
        removed: None,
    }
}

fn project_open(
    studio: &mut Studio,
    ctx: &CallCtx,
    a: ProjectOpenArgs,
) -> Result<ProjectInfo, SurfaceError> {
    let action = match a.asset {
        Some(action) => Some(action),
        None if a.file.is_some() || a.data.is_some() || a.name.is_some() => {
            return Err(SurfaceError::invalid_argument(
                "`file`, `data` and `name` go with `asset`".to_owned(),
            ));
        }
        None => None,
    };
    // Listing only reads; the registry checked `inspect`, the rest is
    // checked here, refused the way the registry does.
    if matches!(action, Some(AssetAction::Import | AssetAction::Remove))
        && !ctx.caps.contains(Cap::Fs)
    {
        return Err(denied("project_open", Cap::Fs, ctx));
    }
    if let Some(path) = a.path {
        studio.switch(&path)?;
    }
    let mut out = ProjectInfo {
        documents: Vec::new(),
        ..info(studio)
    };
    match action {
        Some(AssetAction::List) => out.assets = Some(assets::list(studio.session.store())),
        Some(AssetAction::Import) => {
            out.asset = Some(assets::import(
                studio,
                a.file.as_deref(),
                a.data.as_deref(),
                a.name.as_deref(),
            )?);
        }
        Some(AssetAction::Remove) => {
            let name = a.name.ok_or_else(|| {
                SurfaceError::invalid_argument("`remove` needs `name`".to_owned())
            })?;
            out.removed = Some(assets::remove(studio, &name)?);
        }
        None => {}
    }
    out.documents = studio.session.documents();
    Ok(out)
}

pub(crate) const MUTATE: ToolAnnotations = ToolAnnotations {
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
    /// Also answer `types`, the toolkit types this document can place.
    doc: Option<String>,
    /// Only blocks and types whose name holds this, in any case.
    search: Option<String>,
    /// From a previous page's `next`.
    cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct BlocksOut {
    /// By module, then name: `module`, `name`, `version`, `origin`,
    /// `category`?, `summary`?, and `state`, `inputs`, `actions`, `events`,
    /// `slots` as `ljos_blocks` writes them.
    blocks: Vec<serde_json::Value>,
    /// The toolkit types `doc` can place, by module.
    types: Vec<crate::toolkit::Group>,
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

fn literal(value: LiteralArg) -> Literal {
    match value {
        LiteralArg::Bool(b) => Literal::Bool(b),
        LiteralArg::Number(n) => Literal::Number(n.to_string()),
        LiteralArg::String(s) => Literal::String(s),
    }
}

/// The id of the object `address` names: states name objects by id.
fn id_of(studio: &Mutex<Studio>, doc: &str, address: &Address) -> Result<String, SurfaceError> {
    let (handle, node) = lock(studio).session.resolve(doc, address).map_err(error)?;
    node.id.ok_or_else(|| {
        error(studio_core::Error::NoId(format!(
            "{handle} (`{}`)",
            node.ty
        )))
    })
}

/// The root of `doc` by its handle, for an edit that leaves its object
/// out: by path it would need the generation the path was read at, which
/// a call that names no object has no reason to give.
fn root(studio: &mut Studio, doc: &str) -> Result<Address, SurfaceError> {
    let (handle, _) = studio
        .session
        .resolve(doc, &Address::Path("root".to_owned()))
        .map_err(error)?;
    Ok(Address::Node(handle))
}

/// A `JsonListModel` document `doc` reading the project's JSON file `data`,
/// which must be an array of records: what a list of it shows is then
/// what the file says, whatever saves it.
fn json_model(doc: &str, data: &str, json: &str) -> Result<String, SurfaceError> {
    studio_core::models::records(json).map_err(|e| {
        SurfaceError::invalid_argument(format!("`{data}` is not an array of records: {e}"))
            .with_details(serde_json::json!({ "kind": "not_a_list" }))
    })?;
    let source = Literal::String(studio_core::models::relative_url(doc, data)).to_qml();
    // Resolved against this file whoever makes the model: a relative
    // `source` is the file's, and saying so keeps it so when the model is
    // made by a document elsewhere.
    Ok(format!(
        "import Ljos.Models\n\nJsonListModel {{\n    source: Qt.resolvedUrl({source})\n}}\n"
    ))
}

/// A `ListModel` document holding `json`'s records, each a `ListElement`
/// whose roles are the record's keys in the order written.
fn list_model(path: &str, json: &str) -> Result<String, SurfaceError> {
    let refuse = |kind: &str, why: String| {
        SurfaceError::invalid_argument(format!("`{path}` {why}"))
            .with_details(serde_json::json!({ "kind": kind }))
    };
    let records = studio_core::models::records(json)
        .map_err(|e| refuse("not_a_list", format!("is not an array of records: {e}")))?;
    if records.is_empty() {
        return Ok("import QtQuick\n\nListModel {\n}\n".to_owned());
    }
    let mut out = String::from("import QtQuick\n\nListModel {\n");
    for record in &records {
        out.push_str("    ListElement {\n");
        for (n, (role, value)) in record.0.iter().enumerate() {
            let first_lower = role.chars().next().is_some_and(|c| c.is_ascii_lowercase());
            if !first_lower || !role.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(refuse(
                    "not_a_role",
                    format!("has the key `{role}`; a role is a name starting lowercase"),
                ));
            }
            // Written `id: …`, QML reads it as the element's id, not a role.
            if role == "id" {
                return Err(refuse(
                    "not_a_role",
                    "has the key `id`, which QML reads as an object's id; name the role \
                     otherwise (`key`, `uid`)"
                        .to_owned(),
                ));
            }
            // QML refuses a property set twice in one object.
            if record.0[..n].iter().any(|(r, _)| r == role) {
                return Err(refuse(
                    "repeated_role",
                    format!("has the key `{role}` twice in one record"),
                ));
            }
            let literal = match value {
                serde_json::Value::String(s) => Literal::String(s.clone()),
                serde_json::Value::Number(n) => Literal::Number(n.to_string()),
                serde_json::Value::Bool(b) => Literal::Bool(*b),
                _ => {
                    return Err(refuse(
                        "not_flat",
                        format!("has `{role}: {value}`; a row holds strings, numbers and booleans"),
                    ));
                }
            };
            out.push_str(&format!("        {role}: {}\n", literal.to_qml()));
        }
        out.push_str("    }\n");
    }
    out.push_str("}\n");
    Ok(out)
}

/// Refuses a handler of a signal the trigger's type does not have, naming
/// those it has. One whose type is described nowhere is written as asked:
/// what it has is not known here, and Ljós says so when it compiles.
fn has_signal(
    studio: &Mutex<Studio>,
    doc: &str,
    trigger: &Address,
    signal: &str,
) -> Result<(), SurfaceError> {
    // What is not a name is no signal of any type: the edit refuses it as
    // such.
    if !studio_core::flows::is_identifier(signal) {
        return Ok(());
    }
    let mut studio = lock(studio);
    // An object that is gone is the edit's to answer for: it can say which
    // writer took it out.
    let Ok((handle, node)) = studio.session.resolve(doc, trigger) else {
        return Ok(());
    };
    let source = studio.session.text(doc).map_err(error)?.0.to_owned();
    let Some(signals) = crate::signals::of(studio.session.store(), doc, &source, &node) else {
        return Ok(());
    };
    if signals.iter().any(|s| s.name == signal) {
        return Ok(());
    }
    let offered: Vec<&str> = signals
        .iter()
        .filter(|s| !s.change)
        .map(|s| s.name.as_str())
        .collect();
    let has = if offered.is_empty() {
        "none but its properties' change signals".to_owned()
    } else {
        format!("{} and its properties' change signals", offered.join(", "))
    };
    Err(SurfaceError::new(
        ErrorCode::NotFound,
        format!(
            "{handle} (`{}`) has no signal `{signal}`; it has {has}",
            node.ty
        ),
    )
    .with_details(json!({
        "kind": "no_signal",
        "signal": signal,
        "type": node.ty,
        "signals": offered,
    })))
}

fn needs(field: &str, action: &str) -> SurfaceError {
    SurfaceError::invalid_argument(format!("`{action}` needs `{field}`"))
}

/// The edit a `state_edit` call asks for, with a breakpoint turned into the
/// condition it stands for.
fn state_op(studio: &Mutex<Studio>, a: StateEditArgs) -> Result<Op, SurfaceError> {
    match a.action {
        StateAction::Interaction => {
            let trigger = a.trigger.ok_or_else(|| needs("trigger", "interaction"))?;
            let goal = match (a.state, a.open) {
                (Some(_), Some(_)) => {
                    return Err(SurfaceError::invalid_argument(
                        "give `state` or `open`, not both",
                    ));
                }
                (Some(state), None) => Goal::State {
                    owner: a.node,
                    state,
                },
                (None, Some(doc)) => Goal::Open {
                    navigator: a
                        .navigator
                        .ok_or_else(|| needs("navigator", "interaction"))?,
                    doc,
                },
                (None, None) => return Err(needs("state` or `open", "interaction")),
            };
            if let Some(signal) = &a.signal {
                has_signal(studio, &a.doc, &trigger, signal)?;
            }
            return Ok(Op::Interaction {
                trigger,
                signal: a.signal,
                goal,
            });
        }
        StateAction::RemoveInteraction => {
            return Ok(Op::RemoveInteraction {
                trigger: a
                    .trigger
                    .ok_or_else(|| needs("trigger", "remove_interaction"))?,
                signal: a.signal,
            });
        }
        _ => {}
    }
    let node = match a.node {
        Some(node) => node,
        None => root(&mut lock(studio), &a.doc)?,
    };
    let state = |action: &str| a.state.clone().ok_or_else(|| needs("state", action));
    let when = match (a.when, a.breakpoint) {
        (Some(_), Some(_)) => {
            return Err(SurfaceError::invalid_argument(
                "give `when` or `breakpoint`, not both",
            ));
        }
        (when, None) => when,
        (None, Some(target)) => {
            // The bound compares the root's width: the window a breakpoint
            // is about, not the object the state belongs to.
            let root = id_of(studio, &a.doc, &Address::Path("root".to_owned()))?;
            Some(target.condition(&root))
        }
    };
    let edit = match a.action {
        StateAction::Add => StateEdit::Add {
            name: state("add")?,
            when,
        },
        StateAction::Rename => StateEdit::Rename {
            name: state("rename")?,
            to: a.new_name.ok_or_else(|| needs("new_name", "rename"))?,
        },
        StateAction::Remove => StateEdit::Remove {
            name: state("remove")?,
        },
        StateAction::When => StateEdit::When {
            name: state("when")?,
            when,
        },
        StateAction::Change => {
            let target = a.target.ok_or_else(|| needs("target", "change"))?;
            let value = match (a.value, a.binding) {
                (Some(_), Some(_)) => {
                    return Err(SurfaceError::invalid_argument(
                        "give `value` or `binding`, not both",
                    ));
                }
                (Some(v), None) => Some(literal(v).to_qml()),
                (None, binding) => binding,
            };
            StateEdit::Change {
                name: state("change")?,
                target: id_of(studio, &a.doc, &target)?,
                property: a.name.ok_or_else(|| needs("name", "change"))?,
                value,
            }
        }
        StateAction::AddTransition => StateEdit::AddTransition {
            from: a.from.unwrap_or_else(|| "*".to_owned()),
            to: a.to.unwrap_or_else(|| "*".to_owned()),
            animations: a
                .animations
                .ok_or_else(|| needs("animations", "add_transition"))?,
        },
        StateAction::RemoveTransition => StateEdit::RemoveTransition {
            from: a.from.unwrap_or_else(|| "*".to_owned()),
            to: a.to.unwrap_or_else(|| "*".to_owned()),
        },
        StateAction::Interaction | StateAction::RemoveInteraction => {
            unreachable!("handled above")
        }
    };
    Ok(Op::State { node, edit })
}

/// The edit a `slot_contribute` call asks for.
fn contribute_op(studio: &Mutex<Studio>, a: &ContributeArgs) -> Result<Op, SurfaceError> {
    if let Some(node) = &a.remove {
        if a.slot.is_some() || a.text.is_some() || a.block.is_some() {
            return Err(SurfaceError::invalid_argument(
                "`remove` takes a contribution out; leave `slot`, `text` and `block` out",
            ));
        }
        return Ok(Op::RemoveContribution { node: node.clone() });
    }
    let slot = a
        .slot
        .clone()
        .ok_or_else(|| SurfaceError::invalid_argument("give the `slot`, or `remove`"))?;
    let mut studio = lock(studio);
    let registry = blocks::registry(studio.session.store());
    // Slot names are global (ADR 0003): any block that offers one makes a
    // contribution to it meaningful.
    if !registry
        .blocks()
        .any(|b| b.slots.iter().any(|s| s.name == slot))
    {
        return Err(SurfaceError::not_found(format!(
            "no block offers a slot `{slot}`; `blocks_list` lists them"
        ))
        .with_details(json!({ "kind": "no_slot" })));
    }
    let mut imports = vec![blocks::RUNTIME.to_owned()];
    let delegate = match (&a.text, &a.block) {
        (Some(_), Some(_)) => {
            return Err(SurfaceError::invalid_argument(
                "give the delegate's `text` or a `block`, not both",
            ));
        }
        (None, None) => {
            return Err(SurfaceError::invalid_argument(
                "give the delegate's `text` or a `block`",
            ));
        }
        (Some(text), None) => text.clone(),
        (None, Some(qualified)) => {
            let Some(block) = blocks::find(&registry, qualified) else {
                return Err(SurfaceError::not_found(format!(
                    "no block `{qualified}`; `blocks_list` lists them"
                ))
                .with_details(json!({ "kind": "no_block" })));
            };
            imports.push(block.module.clone());
            format!("{} {{}}", block.name)
        }
    };
    let (text, _) = studio.session.text(&a.doc).map_err(error)?;
    let unit = studio_core::states::unit(text);
    let text = studio_core::slots::contribution(&slot, a.mode, a.order, &delegate, &unit)
        .map_err(error)?;
    Ok(Op::Contribute {
        parent: match a.parent.clone() {
            Some(parent) => parent,
            None => root(&mut studio, &a.doc)?,
        },
        slot,
        text,
        imports,
    })
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

/// Sets a binding. A property the object declares with a value has its value
/// there: a binding beside it would set it twice, which the app refuses. An
/// alias has its target there, which stays.
fn set(
    studio: &Mutex<Studio>,
    doc: &str,
    generation: Option<u64>,
    op: &Op,
    dry_run: bool,
) -> Result<EditOut, SurfaceError> {
    let (node, name, value) = match op {
        Op::SetBinding { node, name, value } => (node, name, Some(value)),
        Op::RemoveBinding { node, name } => (node, name, None),
        _ => return edit(studio, doc, generation, op, dry_run),
    };
    let mut studio = lock(studio);
    // An address that names nothing is the edit's to refuse, as it refuses it.
    let object = studio
        .session
        .resolve(doc, node)
        .ok()
        .map(|(_, object)| object);
    let declared = object
        .iter()
        .flat_map(|object| &object.bindings)
        .find(|b| b.declared && !b.alias && b.name == *name);
    let Some(declared) = declared else {
        return studio
            .session
            .edit(doc, generation, op, dry_run)
            .map(EditOut::from)
            .map_err(error);
    };
    // DECISION: refused, not replaced. It would throw away an object and its
    // members for a value, which `Component` and the like do not even take.
    if declared.holds_objects {
        return Err(SurfaceError::new(
            ErrorCode::NotActionable,
            format!("`{name}` holds an object: change it in the code"),
        )
        .with_details(json!({ "kind": "holds_objects" })));
    }
    // The edit would answer that there is no such binding, beside a row that
    // shows it. A declaration is not a binding to take away.
    let Some(value) = value else {
        return Err(SurfaceError::new(
            ErrorCode::NotActionable,
            format!("`{name}` is declared with this value: take it away in the code"),
        )
        .with_details(json!({ "kind": "declared" })));
    };
    // Whether the value can be written at all, and by this writer.
    studio
        .session
        .edit(doc, generation, op, true)
        .map_err(error)?;
    // The range is of the current text: an older generation has to read again.
    let current = studio.session.text(doc).map_err(error)?.1;
    let range = (generation.is_none_or(|g| g == current)).then_some(declared.start..declared.end);
    studio
        .session
        .replace_text(doc, generation, range, value, dry_run)
        .map(EditOut::from)
        .map_err(error)
}

pub(crate) fn registry(studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let registry = crate::place_tools::add(edits(studio), studio);
    let registry = crate::preview_tools::add(registry, studio);
    crate::review::add(crate::export::add(registry, studio), studio)
}

/// The project and edit tools, in wire order.
fn edits(studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let s = || studio.clone();
    ToolRegistry::new()
        .with(
            ToolDef::new::<ProjectOpenArgs>(
                "project_open",
                "Open a project directory below the workspace, or describe the open one. Opening starts a new session: handles and history end. `asset` lists, imports or removes the project's pictures and fonts.",
            )
            .answers::<ProjectInfo>()
            .requires(Cap::Inspect)
            // An asset's import or removal writes; a reader is told by `fs`.
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |ctx: &CallCtx, a: ProjectOpenArgs| project_open(&mut lock(&studio), ctx, a)
            }),
        )
        .with(
            ToolDef::new::<TreeArgs>(
                "document_tree",
                "A document's objects in order: handle, path, type, id, bindings. Grouped properties (`font.bold`) are bindings. Paginated by `cursor`.",
            )
            .answers::<Tree>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: TreeArgs| {
                    let mut studio = lock(&studio);
                    let doc = studio.session.canonical(&a.doc).map_err(error)?;
                    let (tree, generation) = studio.session.tree(&doc).map_err(error)?;
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
                        doc,
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
            .answers::<Text>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: TextArgs| {
                    let mut studio = lock(&studio);
                    let doc = studio.session.canonical(&a.doc).map_err(error)?;
                    let parses = studio.session.tree(&doc).is_ok();
                    let (text, generation) = studio.session.text(&doc).map_err(error)?;
                    let start = a.start.unwrap_or(0);
                    let end = a.end.unwrap_or(text.len());
                    let slice = text.get(start..end).ok_or_else(|| {
                        SurfaceError::invalid_argument(format!("{start}..{end} is not a range of the text"))
                    })?;
                    Ok(Text {
                        doc,
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
                "Create a QML document from `text`, or from a JSON file's records (`data`). Refused if it \
                 exists or does not parse. Undo removes it.",
            )
            .answers::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(MUTATE),
            typed({
                let studio = s();
                move |_, a: NewArgs| {
                    let mut studio = lock(&studio);
                    let text = match (a.text, a.data) {
                        (Some(_), Some(_)) => {
                            return Err(SurfaceError::invalid_argument(
                                "give `text` or `data`, not both",
                            ));
                        }
                        (Some(text), None) => text,
                        (None, Some(data)) => {
                            let json = studio.session.store().read(&data).map_err(store_error)?;
                            match a.model {
                                DataModel::Json => json_model(&a.doc, &data, &json)?,
                                DataModel::List => list_model(&data, &json)?,
                            }
                        }
                        (None, None) => NEW_DOCUMENT.to_owned(),
                    };
                    studio
                        .session
                        .create(&a.doc, &text, a.dry_run)
                        .map(EditOut::from)
                        .map_err(error)
                }
            }),
        )
        .with(
            ToolDef::new::<EditTextArgs>(
                "document_edit",
                "Replace a byte range of a document's text, as the code view does. It may leave the text unparsed; structural edits then wait and handles hold. Needs the generation read.",
            )
            .answers::<EditOut>()
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
                    let now = studio.timer.now();
                    let session = &mut studio.session;
                    if a.typing && !a.dry_run {
                        session.type_text(&a.doc, a.generation, range, &a.text, now)
                    } else {
                        session.replace_text(&a.doc, Some(a.generation), range, &a.text, a.dry_run)
                    }
                    .map(EditOut::from)
                        .map_err(error)
                }
            }),
        )
        .with(
            ToolDef::new::<FindArgs>(
                "node_find",
                "Find objects by id, type, binding name or text in a binding's value, in one document or all (naming those it could not read). Returns handles.",
            )
            .answers::<FindOut>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: FindArgs| {
                    let mut studio = lock(&studio);
                    let docs = match &a.doc {
                        Some(doc) => vec![studio.session.canonical(doc).map_err(error)?],
                        None => studio.session.documents(),
                    };
                    // A file the project reaches by two names is searched once.
                    let mut seen = std::collections::BTreeSet::new();
                    let mut out = FindOut {
                        nodes: Vec::new(),
                        broken: Vec::new(),
                        unreadable: Vec::new(),
                    };
                    for doc in docs {
                        let doc = match studio.session.canonical(&doc) {
                            Ok(real) => real,
                            Err(studio_core::Error::Store(_)) if a.doc.is_none() => {
                                out.unreadable.push(doc);
                                continue;
                            }
                            Err(e) => return Err(error(e)),
                        };
                        if !seen.insert(doc.clone()) {
                            continue;
                        }
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
                "Set Studio's selection (canvas and inspector) by addresses, or by a point `at` in the document's preview. Empty clears it.",
            )
            .answers::<Selection>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |ctx: &CallCtx, a: SelectArgs| {
                    // Holding changes what others may write: checked here,
                    // refused the way the registry does.
                    if a.hold && !ctx.caps.contains(Cap::Mutate) {
                        return Err(denied("node_select", Cap::Mutate, ctx));
                    }
                    let mut studio = lock(&studio);
                    match a.at {
                        Some(_) if !a.nodes.is_empty() => {
                            return Err(SurfaceError::invalid_argument(
                                "give `nodes` or a point `at`, not both",
                            ));
                        }
                        Some(at) => match crate::preview_tools::pick(&mut studio, &a.doc, at.x, at.y)? {
                            Some((doc, handle)) if a.add => {
                                studio
                                    .session
                                    .toggle_selected(&doc, &[Address::Node(handle)])
                                    .map_err(error)?;
                            }
                            Some((doc, handle)) => {
                                studio
                                    .session
                                    .select(&doc, &[Address::Node(handle)])
                                    .map_err(error)?;
                            }
                            // A shift-click that misses keeps what is selected.
                            None if a.add => {}
                            None => {
                                studio.session.select(&a.doc, &[]).map_err(error)?;
                            }
                        },
                        None if a.add => {
                            studio
                                .session
                                .toggle_selected(&a.doc, &a.nodes)
                                .map_err(error)?;
                        }
                        None => {
                            studio.session.select(&a.doc, &a.nodes).map_err(error)?;
                        }
                    }
                    let mut selection = Vec::new();
                    for (doc, handle) in studio.session.selection() {
                        let (handle, node) = studio
                            .session
                            .resolve(&doc, &Address::Node(handle))
                            .map_err(error)?;
                        selection.push(found(&doc, handle, &node));
                    }
                    // A point can select an object declared in another file,
                    // and a lease is on the document that object is in.
                    let doc = match selection.first() {
                        Some(first) => first.doc.clone(),
                        None => studio.session.canonical(&a.doc).map_err(error)?,
                    };
                    if selection.is_empty() {
                        studio.session.release();
                    } else if a.hold {
                        let handles: Vec<String> =
                            selection.iter().map(|f| f.node.clone()).collect();
                        studio.session.hold(&doc, &handles).map_err(error)?;
                    }
                    let held = studio.session.held(&doc);
                    Ok(Selection { selection, held })
                }
            }),
        )
        .with(
            ToolDef::new::<NodeArgs>(
                "node_inspect",
                "An object's bindings with byte ranges, parent, children, states, transitions, flows, signals with arguments, and the roles and rows of the data it sees (in a delegate, its list's).",
            )
            .answers::<Inspected>()
            .requires(Cap::Inspect),
            typed({
                let studio = s();
                move |_, a: NodeArgs| {
                    let mut studio = lock(&studio);
                    let doc = studio.session.canonical(&a.doc).map_err(error)?;
                    let (handle, node) = studio.session.resolve(&doc, &a.node).map_err(error)?;
                    let variants = studio.session.variants(&doc, &a.node).map_err(error)?;
                    let interactions = studio.session.interactions(&doc, &a.node).map_err(error)?;
                    let model = studio.session.model(&doc, &a.node).map_err(error)?;
                    let source = studio.session.text(&doc).map_err(error)?.0.to_owned();
                    let crate::signals::TypeSays { signals, takes, .. } =
                        crate::signals::described(studio.session.store(), &doc, &source, &node);
                    let (tree, generation) = studio.session.tree(&doc).map_err(error)?;
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
                        .map(|(h, n)| found(&doc, h.clone(), n))
                        .collect();
                    Ok(Inspected {
                        doc,
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
                        states: variants.states,
                        transitions: variants.transitions,
                        interactions,
                        model,
                        signals,
                        takes,
                    })
                }
            }),
        )
        .with(
            ToolDef::new::<BlocksArgs>(
                "blocks_list",
                "List the project's functional blocks with their contracts; `node_insert` places one by `Module.Name`. Unreadable ones are in `errors`. With `doc`, `types` lists the toolkit types it can place, its imports first.",
            )
            .answers::<BlocksOut>()
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
                    let search = a.search.as_deref().unwrap_or_default();
                    let mut all = list("blocks");
                    all.retain(|b| {
                        b["name"].as_str().is_some_and(|n| {
                            n.to_lowercase().contains(&search.to_lowercase())
                        })
                    });
                    let (blocks, next) = paginate(all, a.cursor.as_deref(), BLOCKS_PAGE)?;
                    let types = match &a.doc {
                        Some(doc) => {
                            let mut studio = lock(&studio);
                            let (text, _) = studio.session.text(doc).map_err(error)?;
                            crate::toolkit::offered(text, search)
                        }
                        None => Vec::new(),
                    };
                    Ok(BlocksOut {
                        blocks,
                        types,
                        errors: list("errors"),
                        next,
                    })
                }
            }),
        )
        .with(
            ToolDef::new::<InsertArgs>(
                "node_insert",
                "Insert an object written in QML, or a `block` from `blocks_list`, as a child at an index, or a member (function, signal, property, handler) of `parent`. It gets an id unless it has one. Returns the handle.",
            )
            .answers::<EditOut>()
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
            .answers::<EditOut>()
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
            .answers::<EditOut>()
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
                "Set a property to a literal (bool, number, string), or remove it with null. Grouped and dotted forms are one property; a new one follows the file's style.",
            )
            .answers::<EditOut>()
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
                        Some(value) => Op::SetBinding {
                            node: a.node,
                            name: a.name,
                            value: literal(value).to_qml(),
                        },
                    };
                    set(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<BindingArgs>(
                "binding_set",
                "Set a binding expression or a signal handler, or remove it with null. Refused \
                 with `would_break` if the file would not parse.",
            )
            .answers::<EditOut>()
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
                    set(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<StateEditArgs>(
                "state_edit",
                "States: add, rename, remove; change a value or the `when` (`breakpoint`: the root's width bound); add or remove a Transition; write or remove a flow (a tap on `trigger` goes to `state` or opens a document). `would_break` if the file would not parse.",
            )
            .answers::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |_, a: StateEditArgs| {
                    let (doc, generation, dry_run) = (a.doc.clone(), a.generation, a.dry_run);
                    let op = state_op(&studio, a)?;
                    edit(&studio, &doc, generation, &op, dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<ContributeArgs>(
                "slot_contribute",
                "Write a Contribution to a block's slot, showing QML `text` or a `block` (imports added once); or `remove` one. Refuses a slot no block offers (`blocks_list`).",
            )
            .answers::<EditOut>()
            .requires(Cap::Mutate)
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |_, a: ContributeArgs| {
                    let op = contribute_op(&studio, &a)?;
                    edit(&studio, &a.doc, a.generation, &op, a.dry_run)
                }
            }),
        )
        .with(
            ToolDef::new::<HistoryArgs>(
                "history",
                "List edits and their authors (`inspect`), or undo or redo the caller's last (`mutate`), moved past others' edits; `all` takes the newest whoever made it. Undo gives back the document's bytes.",
            )
            .answers::<HistoryOut>()
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
                        HistoryAction::Undo if a.all => {
                            Some(studio.session.undo_newest().map_err(error)?)
                        }
                        HistoryAction::Undo => Some(studio.session.undo().map_err(error)?),
                        HistoryAction::Redo if a.all => {
                            Some(studio.session.redo_newest().map_err(error)?)
                        }
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
        .with(
            ToolDef::new::<ThemeArgs>(
                "theme_set",
                "Read the tokens of Ljos.Design, the module's value beside the theme file's; or set one (`null` gives it back) in the theme file (made if none), the rest kept byte for byte. A theme Ljós would refuse is refused.",
            )
            .answers::<ThemeOut>()
            .requires(Cap::Mutate)
            .annotated(DESTRUCTIVE),
            typed({
                let studio = s();
                move |_, a: ThemeArgs| theme_set(&studio, a)
            }),
        )
}
