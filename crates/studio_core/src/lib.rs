//! Ljós Studio's session: the documents of an open project, the objects in
//! them under addresses that survive edits, the selection and the history.
//!
//! Every way of changing a design goes through [`Session`]: the canvas, the
//! code view and an agent's MCP call alike. So an agent's edit shows on the
//! canvas, a person can undo it, and two writers never silently overwrite
//! each other: an edit names the document generation it was made against
//! and a stale one is refused.
//!
//! The file is the document (DESIGN.md goal 1). Edits are `qml_edit`'s
//! lossless splices, history entries are the changed ranges and their
//! inverses, and undo gives back the bytes.

mod address;
mod history;
pub mod outline;
pub mod preview;
pub mod slots;
pub mod states;
mod store;
pub mod theme;

use std::collections::BTreeMap;

pub use address::{Address, PathChange, parse_path};
pub use history::{Entry, EntrySummary};
pub use outline::{Binding, Node};
pub use qml_edit::{Change, Error as EditError, NodePath};
pub use slots::Mode;
pub use states::{ChangeInfo, StateEdit, StateInfo, TransitionInfo, Variants};
pub use store::{FsStore, MemStore, Store, StoreError};

use address::{Tracked, find_by_key};
use history::History;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("{0}")]
    Edit(#[from] qml_edit::Error),
    #[error("`{doc}` is at generation {current}, not {given}: read it again and redo the edit")]
    Stale {
        doc: String,
        given: u64,
        current: u64,
    },
    #[error("`{doc}` does not parse ({problem}); structural edits wait until it does")]
    Broken { doc: String, problem: String },
    #[error("no object {address} in `{doc}`")]
    NotFound { doc: String, address: String },
    #[error("the object {handle} is gone from `{doc}`")]
    Gone { doc: String, handle: String },
    #[error("`{0}` already exists")]
    Exists(String),
    #[error("{0}")]
    Invalid(String),
    #[error("nothing to {0}")]
    NothingTo(&'static str),
    #[error("no state `{0}`")]
    NoState(String),
    #[error("no transition from `{from}` to `{to}`")]
    NoTransition { from: String, to: String },
    #[error("the state `{state}` does not change `{name}`")]
    NoChange { state: String, name: String },
    /// A state names objects by `id`: one without cannot be changed by it
    /// nor, for the root, compared with a breakpoint.
    #[error("{0} has no `id`; give it one first")]
    NoId(String),
    #[error("{0} is not a `Contribution`")]
    NotAContribution(String),
    #[error("`Ljos.Design` has no token `{0}`")]
    NoToken(String),
    /// Ljós would not load the theme; its reasons, each
    /// `file:line:column: message`.
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))]
    Theme(Vec<theme::ThemeError>),
}

/// A literal a property can be set to.
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Bool(bool),
    /// Written as given: the caller has formatted the number.
    Number(String),
    String(String),
}

impl Literal {
    /// The QML that writes it.
    pub fn to_qml(&self) -> String {
        match self {
            Literal::Bool(b) => b.to_string(),
            Literal::Number(n) => n.clone(),
            Literal::String(s) => {
                let mut out = String::with_capacity(s.len() + 2);
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c => out.push(c),
                    }
                }
                out.push('"');
                out
            }
        }
    }
}

/// A structural edit of one document.
#[derive(Clone, Debug)]
pub enum Op {
    /// Put the object written in `text` into `parent` as child `index`
    /// (the end when `None`). Without an `id` of its own it is given one, so
    /// it keeps a name a person and the code view can see; `import` is a
    /// module the document must import for it.
    Insert {
        parent: Address,
        index: Option<usize>,
        text: String,
        add_id: bool,
        import: Option<String>,
    },
    Move {
        node: Address,
        parent: Address,
        index: Option<usize>,
    },
    Remove {
        node: Address,
    },
    /// `value` is QML as written after the `:`: a literal, an expression or
    /// a handler's block.
    SetBinding {
        node: Address,
        name: String,
        value: String,
    },
    RemoveBinding {
        node: Address,
        name: String,
    },
    /// A change to `node`'s states or transitions.
    State {
        node: Address,
        edit: StateEdit,
    },
    /// Put a `Contribution` to `slot`, written in `text`, into `parent`
    /// last, with an `id`, importing each of `imports`.
    Contribute {
        parent: Address,
        slot: String,
        text: String,
        imports: Vec<String>,
    },
    /// Take out a `Contribution`; refused for any other object.
    RemoveContribution {
        node: Address,
    },
}

impl Op {
    fn addresses(&self) -> Vec<&Address> {
        match self {
            Op::Insert { parent, .. } => vec![parent],
            Op::Move { node, parent, .. } => vec![node, parent],
            Op::Remove { node }
            | Op::SetBinding { node, .. }
            | Op::RemoveBinding { node, .. }
            | Op::State { node, .. }
            | Op::RemoveContribution { node } => vec![node],
            Op::Contribute { parent, .. } => vec![parent],
        }
    }

    fn label(&self) -> String {
        match self {
            Op::Insert { text, .. } => {
                format!(
                    "insert {}",
                    text.split(['{', ' ']).next().unwrap_or("object").trim()
                )
            }
            Op::Move { .. } => "move".to_owned(),
            Op::Remove { .. } => "remove".to_owned(),
            Op::SetBinding { name, .. } => format!("set {name}"),
            Op::RemoveBinding { name, .. } => format!("remove {name}"),
            Op::State { edit, .. } => edit.label(),
            Op::Contribute { slot, .. } => format!("contribute to {slot}"),
            Op::RemoveContribution { .. } => "remove contribution".to_owned(),
        }
    }
}

/// What an edit did.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub doc: String,
    pub generation: u64,
    /// Ranges of the text before the edit and what replaced them.
    pub changes: Vec<Change>,
    /// The history entry, `None` for a dry run or an unchanged document.
    pub history: Option<u64>,
    /// The handle of the object the edit put or moved, if it did.
    pub node: Option<String>,
    /// The new text, for a dry run.
    pub text: Option<String>,
    /// The document already said this: nothing was written and no history
    /// entry made.
    pub unchanged: bool,
}

/// The tokens [`Session::tokens`] read.
#[derive(Clone, Debug)]
pub struct Tokens {
    /// The theme file, as the session names it.
    pub file: String,
    pub exists: bool,
    /// The file's generation, 0 while it does not exist.
    pub generation: u64,
    /// Why Ljós would not load the file, when it would not; its values
    /// are then left out.
    pub problem: Option<String>,
    pub tokens: Vec<theme::Token>,
}

/// The type a slot's elements have (`ljos_blocks::CONTRIBUTION`).
const CONTRIBUTION: &str = "Contribution";

/// One open document.
#[derive(Clone, Debug)]
struct Document {
    text: String,
    generation: u64,
    /// `None` while the text does not parse; handles then wait.
    nodes: Option<Vec<Node>>,
    problem: Option<String>,
    tracked: Vec<Tracked>,
}

impl Document {
    /// `past` is the last generation an earlier life of this path had: one
    /// read then can never pass for current now.
    fn new(text: String, past: u64) -> Self {
        let mut doc = Self {
            text: String::new(),
            generation: past,
            nodes: None,
            problem: None,
            tracked: Vec::new(),
        };
        doc.set_text(text);
        doc
    }

    fn set_text(&mut self, text: String) {
        self.nodes = outline::outline(&text);
        self.problem = if self.nodes.is_none() {
            Some(problem(&text))
        } else {
            None
        };
        self.text = text;
        self.generation += 1;
    }
}

fn problem(text: &str) -> String {
    match qml_edit::objects(text) {
        Err(e) => e.to_string(),
        Ok(_) => "unknown".to_owned(),
    }
}

/// A project open in Studio.
pub struct Session {
    store: Box<dyn Store>,
    docs: BTreeMap<String, Document>,
    history: History,
    selection: Vec<(String, String)>,
    next_handle: u64,
    /// The last generation of documents no longer held.
    retired: BTreeMap<String, u64>,
    /// Told the path of every file the session wrote or removed, after it
    /// did: what shows the project (a preview) follows edits through it
    /// rather than by watching the files.
    on_write: Vec<OnWrite>,
}

/// What [`Session::on_write`] is given.
type OnWrite = Box<dyn FnMut(&str) + Send>;

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("store", &self.store.describe())
            .field("docs", &self.docs.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

/// Whether `name` can be a binding's name: dotted identifiers.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

impl Session {
    pub fn open(store: Box<dyn Store>) -> Self {
        Self {
            store,
            docs: BTreeMap::new(),
            history: History::default(),
            selection: Vec::new(),
            next_handle: 1,
            retired: BTreeMap::new(),
            on_write: Vec::new(),
        }
    }

    /// Calls `f` with the path of each file the session writes or removes
    /// from now on, after the store has it.
    pub fn on_write(&mut self, f: impl FnMut(&str) + Send + 'static) {
        self.on_write.push(Box::new(f));
    }

    fn wrote(&mut self, path: &str) {
        for f in &mut self.on_write {
            f(path);
        }
    }

    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    /// The project's QML documents.
    pub fn documents(&self) -> Vec<String> {
        self.store.list(".qml")
    }

    /// A document [`Session::sync`] has brought up to date.
    fn doc(&mut self, path: &str) -> Result<&mut Document, Error> {
        self.docs
            .get_mut(path)
            .ok_or_else(|| StoreError::Missing(path.to_owned()).into())
    }

    /// Brings the held document level with the file, which other programs
    /// write too. Every public way in calls this once, so nothing is read or
    /// edited on text the file no longer has.
    ///
    /// It does not tell a write observer: this is a change the file already
    /// has, and whatever watches the file has seen it.
    fn sync(&mut self, path: &str) -> Result<(), Error> {
        self.check_document(path)?;
        let stored = match self.store.read(path) {
            Ok(stored) => stored,
            Err(e @ StoreError::Missing(_)) => {
                // Only a document held went away. One undo removed is not
                // held, and redo can still bring it back.
                if self.docs.contains_key(path) {
                    self.forget(path);
                }
                return Err(e.into());
            }
            Err(e) => return Err(e.into()),
        };
        match self.docs.get(path) {
            None => {
                let past = self.retired.get(path).copied().unwrap_or(0);
                self.docs
                    .insert(path.to_owned(), Document::new(stored, past));
            }
            Some(doc) if doc.text == stored => {}
            Some(doc) => {
                let before = doc.text.clone();
                self.record(
                    path,
                    before,
                    stored,
                    None,
                    "changed outside Studio".to_owned(),
                )?;
            }
        }
        Ok(())
    }

    /// Refuses a path that is not one of the project's documents (its `.qml`
    /// files and theme files), as written or where its links lead: the right
    /// to edit a document is not the right to change `.git/config`.
    pub fn check_document(&self, path: &str) -> Result<(), Error> {
        let real = self.store.resolved(path)?;
        if store::document_path(path) && store::document_path(&real) {
            Ok(())
        } else {
            Err(StoreError::NotDocument(path.to_owned()).into())
        }
    }

    /// The name a document is held under: where `path` really is, links
    /// followed, so one file reached by two names is one document with one
    /// generation and one history. Refuses what [`Session::check_document`]
    /// refuses.
    pub fn canonical(&self, path: &str) -> Result<String, Error> {
        self.check_document(path)?;
        Ok(self.store.resolved(path)?)
    }

    /// Lets go of a document whose file went: its history's offsets have
    /// nothing left to apply to.
    fn forget(&mut self, path: &str) {
        self.retire(path);
        self.history.forget(path);
    }

    fn retire(&mut self, path: &str) {
        if let Some(doc) = self.docs.remove(path) {
            self.retired.insert(path.to_owned(), doc.generation);
        }
        self.selection.retain(|(d, _)| d != path);
    }

    pub fn text(&mut self, path: &str) -> Result<(&str, u64), Error> {
        let path = &self.canonical(path)?;
        self.sync(path)?;
        let doc = self.doc(path)?;
        Ok((&doc.text, doc.generation))
    }

    pub fn generation(&mut self, path: &str) -> Result<u64, Error> {
        let path = &self.canonical(path)?;
        self.sync(path)?;
        Ok(self.doc(path)?.generation)
    }

    /// The document's objects, each with a handle. Fails while the text does
    /// not parse.
    pub fn tree(&mut self, path: &str) -> Result<(Vec<(String, Node)>, u64), Error> {
        let path = &self.canonical(path)?;
        self.sync(path)?;
        self.tree_now(path)
    }

    fn tree_now(&mut self, path: &str) -> Result<(Vec<(String, Node)>, u64), Error> {
        let mut next = self.next_handle;
        let doc = self.doc(path)?;
        let Some(nodes) = &doc.nodes else {
            return Err(Error::Broken {
                doc: path.to_owned(),
                problem: doc.problem.clone().unwrap_or_default(),
            });
        };
        let mut out = Vec::with_capacity(nodes.len());
        let mut given = Vec::new();
        for node in nodes {
            let handle = match doc
                .tracked
                .iter()
                .find(|t| t.path.as_ref() == Some(&node.path))
            {
                Some(t) => t.handle,
                None => {
                    next += 1;
                    let t = Tracked {
                        handle: next - 1,
                        path: Some(node.path.clone()),
                        key: node.key.clone(),
                        ty: node.ty.clone(),
                        span: Some((node.start, node.end)),
                    };
                    given.push(t.clone());
                    doc.tracked.push(t);
                    next - 1
                }
            };
            out.push((format!("n{handle}"), node.clone()));
        }
        let generation = doc.generation;
        self.next_handle = next;
        // A handle given out on the text an edit left is one that edit's
        // redo must put back: an object the edit made has no other way to
        // be found again once undo took it out.
        if let Some(entry) = self.history.latest_mut(path) {
            entry.handles_after.extend(given);
        }
        Ok((out, generation))
    }

    /// The object an address names now, with its handle.
    pub fn resolve(&mut self, path: &str, address: &Address) -> Result<(String, Node), Error> {
        let path = &self.canonical(path)?;
        self.sync(path)?;
        self.resolve_now(path, address)
    }

    fn resolve_now(&mut self, path: &str, address: &Address) -> Result<(String, Node), Error> {
        let (tree, _) = self.tree_now(path)?;
        let not_found = || Error::NotFound {
            doc: path.to_owned(),
            address: match address {
                Address::Node(h) => h.clone(),
                Address::Id(id) => format!("with id `{id}`"),
                Address::Path(p) => format!("at {p}"),
            },
        };
        match address {
            Address::Node(handle) => {
                let n: u64 = handle
                    .strip_prefix('n')
                    .and_then(|n| n.parse().ok())
                    .ok_or_else(not_found)?;
                let doc = self.doc(path)?;
                let tracked = doc
                    .tracked
                    .iter()
                    .find(|t| t.handle == n)
                    .ok_or_else(not_found)?;
                let at = tracked.path.clone().ok_or_else(|| Error::Gone {
                    doc: path.to_owned(),
                    handle: handle.clone(),
                })?;
                tree.into_iter()
                    .find(|(_, node)| node.path == at)
                    .ok_or_else(not_found)
            }
            Address::Id(id) => tree
                .into_iter()
                .find(|(_, node)| node.id.as_deref() == Some(id))
                .ok_or_else(not_found),
            Address::Path(p) => {
                let at =
                    parse_path(p).ok_or_else(|| Error::Invalid(format!("`{p}` is not a path")))?;
                tree.into_iter()
                    .find(|(_, node)| node.path == at)
                    .ok_or_else(not_found)
            }
        }
    }

    /// The states and transitions of the object `address` names.
    ///
    /// # Errors
    /// An address that names nothing, or a document that does not parse.
    pub fn variants(&mut self, path: &str, address: &Address) -> Result<Variants, Error> {
        let (_, node) = self.resolve(path, address)?;
        let text = &self.doc(path)?.text;
        states::variants(text, node.start).ok_or_else(|| Error::NotFound {
            doc: path.to_owned(),
            address: node.path.to_string(),
        })
    }

    /// The object whose type name starts at `line`, `column` (one-based,
    /// in characters, as [`qml_syntax::position`] counts) of `path`: the
    /// one declared there, never one enclosing it. A `column` of 0 means
    /// only the line is known, and then the line must hold one object.
    pub fn declared_at(
        &mut self,
        path: &str,
        line: usize,
        column: usize,
    ) -> Result<(String, Node), Error> {
        let (tree, _) = self.tree(path)?;
        let text = &self.doc(path)?.text;
        let at = |node: &Node| {
            let start = u32::try_from(node.start).unwrap_or(u32::MAX);
            qml_syntax::position(text, start)
        };
        let mut found: Vec<(String, Node)> = tree
            .into_iter()
            .filter(|(_, node)| {
                let (l, c) = at(node);
                l == line && (column == 0 || c == column)
            })
            .collect();
        match found.len() {
            1 => Ok(found.remove(0)),
            0 => Err(Error::NotFound {
                doc: path.to_owned(),
                address: format!("declared at {line}:{column}"),
            }),
            _ => Err(Error::Invalid(format!(
                "line {line} of `{path}` declares {} objects; the column says which",
                found.len()
            ))),
        }
    }

    fn check_generation(&mut self, path: &str, given: Option<u64>) -> Result<(), Error> {
        self.sync(path)?;
        let current = self.doc(path)?.generation;
        match given {
            Some(given) if given != current => Err(Error::Stale {
                doc: path.to_owned(),
                given,
                current,
            }),
            _ => Ok(()),
        }
    }

    /// Applies a structural edit. An edit that names an object by path needs
    /// the `generation` the path was read at: a path is true of one text,
    /// and the only address that would name another object without notice.
    ///
    /// # Errors
    /// A path without a `generation`, a stale `generation`, an address that
    /// names nothing, a document that does not parse, or the edit's own
    /// refusal; nothing changes then.
    pub fn edit(
        &mut self,
        path: &str,
        generation: Option<u64>,
        op: &Op,
        dry_run: bool,
    ) -> Result<Outcome, Error> {
        let path = &self.canonical(path)?;
        if generation.is_none()
            && let Some(Address::Path(p)) = op
                .addresses()
                .into_iter()
                .find(|a| matches!(a, Address::Path(_)))
        {
            return Err(Error::Invalid(format!(
                "the path `{p}` names an object only in the generation it was read at: give \
                 `generation`"
            )));
        }
        self.check_generation(path, generation)?;
        let before = self.doc(path)?.text.clone();
        let (text, change, focus) = self.plan(path, &before, op)?;
        let changes = diff(&before, &text);
        if dry_run {
            return Ok(Outcome {
                doc: path.to_owned(),
                generation: self.doc(path)?.generation,
                unchanged: text == before,
                changes,
                history: None,
                node: None,
                text: Some(text),
            });
        }
        if text == before {
            let node = match focus {
                Some(at) => Some(self.handle_at(path, &at)?),
                None => None,
            };
            return self.unchanged(path, node);
        }
        let entry = self.commit(path, before, text, Some(change), op.label())?;
        let node = match focus {
            Some(at) => Some(self.handle_at(path, &at)?),
            None => None,
        };
        Ok(Outcome {
            doc: path.to_owned(),
            generation: self.doc(path)?.generation,
            changes,
            history: Some(entry),
            node,
            text: None,
            unchanged: false,
        })
    }

    /// The new text, what it does to paths, and where the object it put or
    /// moved ends up.
    fn plan(
        &mut self,
        path: &str,
        before: &str,
        op: &Op,
    ) -> Result<(String, PathChange, Option<NodePath>), Error> {
        {
            let doc = self.doc(path)?;
            if doc.nodes.is_none() {
                return Err(Error::Broken {
                    doc: path.to_owned(),
                    problem: doc.problem.clone().unwrap_or_default(),
                });
            }
        }
        let child_count = |nodes: &[(String, Node)], parent: &NodePath| {
            nodes
                .iter()
                .filter(|(_, n)| {
                    n.path.0.len() == parent.0.len() + 1
                        && n.path.0.starts_with(&parent.0)
                        && matches!(n.path.0.last(), Some(qml_edit::Step::Child(_)))
                })
                .count()
        };
        match op {
            Op::Insert {
                parent,
                index,
                text,
                add_id,
                import,
            } => {
                let (_, parent_node) = self.resolve_now(path, parent)?;
                let (tree, _) = self.tree_now(path)?;
                let count = child_count(&tree, &parent_node.path);
                let index = index.unwrap_or(count);
                let mut text = text.trim().to_owned();
                let imported = match import {
                    Some(module) => imported(before, module),
                    None => Imported::Bare,
                };
                if let Imported::As(qualifier) = &imported {
                    text = qualified(&text, qualifier)?;
                }
                if *add_id {
                    let ids: Vec<String> = tree.iter().filter_map(|(_, n)| n.id.clone()).collect();
                    text = with_id(&text, &ids)?;
                }
                let mut out = qml_edit::apply(
                    before,
                    &qml_edit::Edit::InsertObject {
                        parent: parent_node.path.clone(),
                        index,
                        text,
                    },
                )?
                .text;
                if let (Some(module), Imported::No) = (import, &imported) {
                    out = with_import(&out, module)?;
                }
                Ok((
                    out,
                    PathChange::Insert {
                        parent: parent_node.path.clone(),
                        index,
                    },
                    Some(parent_node.path.child(index)),
                ))
            }
            Op::Move {
                node,
                parent,
                index,
            } => {
                let (_, moved) = self.resolve_now(path, node)?;
                let (_, parent_node) = self.resolve_now(path, parent)?;
                let (tree, _) = self.tree_now(path)?;
                let mut count = child_count(&tree, &parent_node.path);
                // Only a child is one of the children counted; a value
                // object under the same parent is not.
                let same_parent = moved.path.0.len() == parent_node.path.0.len() + 1
                    && moved.path.0.starts_with(&parent_node.path.0)
                    && matches!(moved.path.0.last(), Some(qml_edit::Step::Child(_)));
                if same_parent {
                    count -= 1;
                }
                let index = index.unwrap_or(count);
                let out = qml_edit::apply(
                    before,
                    &qml_edit::Edit::MoveObject {
                        object: moved.path.clone(),
                        parent: parent_node.path.clone(),
                        index,
                    },
                )?
                .text;
                let change = PathChange::Move {
                    object: moved.path.clone(),
                    parent: parent_node.path.clone(),
                    index,
                };
                let at = change.apply(&moved.path);
                Ok((out, change, at))
            }
            Op::Remove { node } => {
                let (_, removed) = self.resolve_now(path, node)?;
                let out = qml_edit::apply(
                    before,
                    &qml_edit::Edit::RemoveObject {
                        object: removed.path.clone(),
                    },
                )?
                .text;
                Ok((
                    out,
                    PathChange::Remove {
                        object: removed.path,
                    },
                    None,
                ))
            }
            Op::SetBinding { node, name, value } => {
                if !valid_name(name) {
                    return Err(Error::Invalid(format!("`{name}` is not a property name")));
                }
                if value.trim().is_empty() {
                    return Err(Error::Invalid("a binding needs a value".to_owned()));
                }
                let (_, target) = self.resolve_now(path, node)?;
                let out = qml_edit::apply(
                    before,
                    &qml_edit::Edit::SetBinding {
                        object: target.path.clone(),
                        name: name.clone(),
                        value: value.clone(),
                    },
                )?
                .text;
                Ok((out, PathChange::Same, None))
            }
            Op::RemoveBinding { node, name } => {
                let (_, target) = self.resolve_now(path, node)?;
                let out = qml_edit::apply(
                    before,
                    &qml_edit::Edit::RemoveBinding {
                        object: target.path.clone(),
                        name: name.clone(),
                    },
                )?
                .text;
                Ok((out, PathChange::Same, None))
            }
            Op::State { node, edit } => {
                let (_, owner) = self.resolve(path, node)?;
                let out = states::apply(before, owner.start, &owner.path, edit)?;
                // States and transitions written in a list are no objects of
                // the outline; one written alone (`states: State {}`) is, and
                // its handle is kept by its key.
                Ok((out, PathChange::Same, None))
            }
            Op::Contribute {
                parent,
                text,
                imports,
                ..
            } => {
                let (text, change, focus) = self.plan(
                    path,
                    before,
                    &Op::Insert {
                        parent: parent.clone(),
                        index: None,
                        text: text.clone(),
                        add_id: true,
                        import: None,
                    },
                )?;
                let mut out = text;
                for module in imports {
                    out = with_import(&out, module)?;
                }
                Ok((out, change, focus))
            }
            Op::RemoveContribution { node } => {
                let (handle, removed) = self.resolve(path, node)?;
                if removed.ty.rsplit('.').next() != Some(CONTRIBUTION) {
                    return Err(Error::NotAContribution(format!(
                        "{handle} (`{}`)",
                        removed.ty
                    )));
                }
                self.plan(path, before, &Op::Remove { node: node.clone() })
            }
        }
    }

    /// The code view's edit: replace `range` of the text (all of it when
    /// `None`) with `text`. The result need not parse; while it does not,
    /// structural edits wait and handles keep their objects for when it
    /// does.
    pub fn replace_text(
        &mut self,
        path: &str,
        generation: Option<u64>,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        dry_run: bool,
    ) -> Result<Outcome, Error> {
        let path = &self.canonical(path)?;
        self.check_generation(path, generation)?;
        let before = self.doc(path)?.text.clone();
        let range = range.unwrap_or(0..before.len());
        if range.start > range.end
            || range.end > before.len()
            || !before.is_char_boundary(range.start)
            || !before.is_char_boundary(range.end)
        {
            return Err(Error::Invalid(format!(
                "{}..{} is not a range of the text",
                range.start, range.end
            )));
        }
        let after = qml_edit::splice(
            &before,
            &[Change {
                range: range.clone(),
                replacement: text.to_owned(),
            }],
        );
        let changes = diff(&before, &after);
        if dry_run {
            return Ok(Outcome {
                doc: path.to_owned(),
                generation: self.doc(path)?.generation,
                unchanged: after == before,
                changes,
                history: None,
                node: None,
                text: Some(after),
            });
        }
        if after == before {
            return self.unchanged(path, None);
        }
        let entry = self.commit(path, before, after, None, "edit text".to_owned())?;
        Ok(Outcome {
            doc: path.to_owned(),
            generation: self.doc(path)?.generation,
            changes,
            history: Some(entry),
            node: None,
            text: None,
            unchanged: false,
        })
    }

    /// A new document with `text`.
    pub fn create(&mut self, path: &str, text: &str, dry_run: bool) -> Result<Outcome, Error> {
        let path = &self.canonical(path)?;
        match self.sync(path) {
            Ok(()) => return Err(Error::Exists(path.to_owned())),
            Err(Error::Store(StoreError::Missing(_))) => {}
            Err(e) => return Err(e),
        }
        if outline::outline(text).is_none() {
            return Err(Error::Broken {
                doc: path.to_owned(),
                problem: problem(text),
            });
        }
        if dry_run {
            return Ok(Outcome {
                doc: path.to_owned(),
                generation: 0,
                changes: vec![Change {
                    range: 0..0,
                    replacement: text.to_owned(),
                }],
                history: None,
                node: None,
                text: Some(text.to_owned()),
                unchanged: false,
            });
        }
        let past = self.retired.get(path).copied().unwrap_or(0);
        let doc = Document::new(text.to_owned(), past);
        let generation = doc.generation;
        self.store.write(path, text)?;
        self.docs.insert(path.to_owned(), doc);
        let entry = self.history.push(Entry {
            id: 0,
            doc: path.to_owned(),
            label: "create".to_owned(),
            forward: vec![Change {
                range: 0..0,
                replacement: text.to_owned(),
            }],
            inverse: vec![Change {
                range: 0..text.len(),
                replacement: String::new(),
            }],
            created: true,
            handles_before: Vec::new(),
            handles_after: Vec::new(),
            change: None,
        });
        self.wrote(path);
        Ok(Outcome {
            doc: path.to_owned(),
            generation,
            changes: vec![Change {
                range: 0..0,
                replacement: text.to_owned(),
            }],
            history: Some(entry),
            node: None,
            text: None,
            unchanged: false,
        })
    }

    /// An edit that would write the bytes already there: the file, the
    /// generation and the history stay as they are.
    fn unchanged(&mut self, path: &str, node: Option<String>) -> Result<Outcome, Error> {
        Ok(Outcome {
            doc: path.to_owned(),
            generation: self.doc(path)?.generation,
            changes: Vec::new(),
            history: None,
            node,
            text: None,
            unchanged: true,
        })
    }

    /// The tokens of `Ljos.Design` (`filter`: one, or one singleton's) with
    /// the theme file's values.
    pub fn tokens(&mut self, file: &str, filter: Option<&str>) -> Result<Tokens, Error> {
        let file = &self.canonical(file)?;
        let exists = self.store.exists(file) || self.docs.contains_key(file);
        let (theme, generation, problem) = if exists {
            let (text, generation) = self.text(file)?;
            match theme::read(file, text) {
                Ok(theme) => (Some(theme), generation, None),
                Err(errors) => (None, generation, Some(Error::Theme(errors).to_string())),
            }
        } else {
            (None, 0, None)
        };
        let tokens = theme::tokens(theme.as_ref(), filter);
        if let Some(filter) = filter
            && tokens.is_empty()
        {
            return Err(Error::NoToken(filter.to_owned()));
        }
        Ok(Tokens {
            file: file.to_owned(),
            exists,
            generation,
            problem,
            tokens,
        })
    }

    /// Sets `token` (in `variant` of a colour) in the theme file `file`,
    /// creating it, or gives it back to the module when `value` is `None`.
    /// Refused unless Ljós would load the result.
    pub fn set_token(
        &mut self,
        file: &str,
        generation: Option<u64>,
        token: &str,
        variant: Option<&str>,
        value: Option<&serde_json::Value>,
        dry_run: bool,
    ) -> Result<Outcome, Error> {
        let file = &self.canonical(file)?;
        let exists = self.store.exists(file) || self.docs.contains_key(file);
        let before = if exists {
            self.check_generation(file, generation)?;
            self.doc(file)?.text.clone()
        } else {
            if let Some(given) = generation.filter(|g| *g != 0) {
                return Err(Error::Stale {
                    doc: file.to_owned(),
                    given,
                    current: 0,
                });
            }
            String::new()
        };
        let after = theme::set(&before, token, variant, value).map_err(|e| match e {
            theme::SetError::NotSet(_) => Error::NothingTo("remove"),
            // Ljós's reasons say where the file is wrong; Studio's only that
            // it cannot edit it.
            theme::SetError::NotJson(_) if theme::read(file, &before).is_err() => {
                Error::Theme(theme::read(file, &before).err().unwrap_or_default())
            }
            e => Error::Invalid(e.to_string()),
        })?;
        theme::check(file, &after).map_err(Error::Theme)?;
        let label = match (value, variant) {
            (None, None) => format!("remove {token}"),
            (None, Some(v)) => format!("remove {token} {v}"),
            (Some(_), None) => format!("set {token}"),
            (Some(_), Some(v)) => format!("set {token} {v}"),
        };
        let changes = diff(&before, &after);
        let unchanged = exists && after == before;
        if dry_run {
            return Ok(Outcome {
                doc: file.to_owned(),
                generation: if exists {
                    self.doc(file)?.generation
                } else {
                    0
                },
                changes,
                history: None,
                node: None,
                text: Some(after),
                unchanged,
            });
        }
        if unchanged {
            return self.unchanged(file, None);
        }
        let entry = if exists {
            self.commit(file, before, after, None, label)?
        } else {
            let past = self.retired.get(file).copied().unwrap_or(0);
            let doc = Document::new(after.clone(), past);
            self.store.write(file, &after)?;
            self.docs.insert(file.to_owned(), doc);
            let entry = self.history.push(Entry {
                id: 0,
                doc: file.to_owned(),
                label,
                forward: changes.clone(),
                inverse: vec![Change {
                    range: 0..after.len(),
                    replacement: String::new(),
                }],
                created: true,
                handles_before: Vec::new(),
                handles_after: Vec::new(),
                change: None,
            });
            self.wrote(file);
            entry
        };
        Ok(Outcome {
            doc: file.to_owned(),
            generation: self.doc(file)?.generation,
            changes,
            history: Some(entry),
            node: None,
            text: None,
            unchanged: false,
        })
    }

    /// Writes the new text, moves the handles and records the entry. The
    /// session's lock outlives a panic (`studio_mcp::lock`), so what can panic
    /// comes first, the parts that must agree together, and the telling last.
    fn commit(
        &mut self,
        path: &str,
        before: String,
        after: String,
        change: Option<PathChange>,
        label: String,
    ) -> Result<u64, Error> {
        let (next, entry) = self.step(path, before, after.clone(), change, label)?;
        self.store.write(path, &after)?;
        *self.doc(path)? = next;
        let id = self.history.push(entry);
        self.wrote(path);
        Ok(id)
    }

    /// Takes `after` as the document's text, which the file already has:
    /// moves the handles and records the entry.
    fn record(
        &mut self,
        path: &str,
        before: String,
        after: String,
        change: Option<PathChange>,
        label: String,
    ) -> Result<u64, Error> {
        let (next, entry) = self.step(path, before, after, change, label)?;
        *self.doc(path)? = next;
        Ok(self.history.push(entry))
    }

    /// The document as it is after the edit, and the edit's entry, worked
    /// out without changing the session.
    fn step(
        &mut self,
        path: &str,
        before: String,
        after: String,
        change: Option<PathChange>,
        label: String,
    ) -> Result<(Document, Entry), Error> {
        let doc = self.doc(path)?;
        let handles_before = doc.tracked.clone();
        let mut next = doc.clone();
        let forward = diff(&before, &after);
        for change in &forward {
            for t in &mut next.tracked {
                t.shift(change);
            }
        }
        next.set_text(after.clone());
        remap(&mut next, change.as_ref());
        let entry = Entry {
            id: 0,
            doc: path.to_owned(),
            label,
            forward,
            inverse: diff(&after, &before),
            created: false,
            handles_before,
            handles_after: next.tracked.clone(),
            change,
        };
        Ok((next, entry))
    }

    fn handle_at(&mut self, path: &str, at: &NodePath) -> Result<String, Error> {
        let (tree, _) = self.tree_now(path)?;
        tree.into_iter()
            .find(|(_, n)| n.path == *at)
            .map(|(h, _)| h)
            .ok_or_else(|| Error::NotFound {
                doc: path.to_owned(),
                address: format!("at {at}"),
            })
    }

    /// Undoes the last edit; its document gets the bytes it had before. The
    /// history moves only once the file has: one that could not be written
    /// can be tried again.
    pub fn undo(&mut self) -> Result<EntrySummary, Error> {
        // A change made outside Studio goes on top here and is what is
        // undone: the entry below it has offsets on a text the file left.
        while let Some(doc) = self.history.peek_undo().map(|e| e.doc.clone()) {
            match self.sync(&doc) {
                Ok(()) => break,
                Err(Error::Store(StoreError::Missing(_))) => self.history.forget(&doc),
                Err(e) => return Err(e),
            }
        }
        let entry = self
            .history
            .peek_undo()
            .cloned()
            .ok_or(Error::NothingTo("undo"))?;
        // It moves only once it has happened: a write that failed leaves it
        // to be tried again, and the lists still say where the file is.
        self.replay(&entry, false)?;
        self.history.commit_undo();
        self.wrote(&entry.doc);
        Ok(entry.summary())
    }

    pub fn redo(&mut self) -> Result<EntrySummary, Error> {
        // A change found here ends what could be redone, like any edit.
        while let Some(top) = self.history.peek_redo() {
            let doc = top.doc.clone();
            if top.created {
                if self.store.exists(&doc) {
                    return Err(Error::Exists(doc));
                }
                break;
            }
            match self.sync(&doc) {
                Ok(()) => break,
                Err(Error::Store(StoreError::Missing(_))) => self.history.forget(&doc),
                Err(e) => return Err(e),
            }
        }
        let entry = self
            .history
            .peek_redo()
            .cloned()
            .ok_or(Error::NothingTo("redo"))?;
        // It moves only once it has happened: a write that failed leaves it
        // to be tried again, and the lists still say where the file is.
        self.replay(&entry, true)?;
        self.history.commit_redo();
        self.wrote(&entry.doc);
        Ok(entry.summary())
    }

    /// Brings the file and the document to that side of `entry`, for the
    /// caller to move the history and tell who watches.
    fn replay(&mut self, entry: &Entry, forward: bool) -> Result<(), Error> {
        let path = entry.doc.clone();
        if entry.created && !forward {
            self.store.remove(&path)?;
            self.retire(&path);
            return Ok(());
        }
        if entry.created && forward {
            let text = qml_edit::splice("", &entry.forward);
            let past = self.retired.get(&path).copied().unwrap_or(0);
            let mut doc = Document::new(text.clone(), past);
            doc.tracked = entry.handles_after.clone();
            self.store.write(&path, &text)?;
            self.docs.insert(path, doc);
            return Ok(());
        }
        let doc = self.doc(&path)?;
        let changes = if forward {
            &entry.forward
        } else {
            &entry.inverse
        };
        let text = qml_edit::splice(&doc.text, changes);
        let mut next = doc.clone();
        for change in &diff(&doc.text, &text) {
            for t in &mut next.tracked {
                t.shift(change);
            }
        }
        next.set_text(text.clone());
        // The handles as they were on that side of the edit: exact, where
        // working them out again from keys could guess.
        let snapshot = if forward {
            &entry.handles_after
        } else {
            &entry.handles_before
        };
        let mut tracked = snapshot.clone();
        let mut leads: Vec<Lead> = tracked
            .iter()
            .map(|t| Lead::Known(t.path.clone()))
            .collect();
        // Handles given out since then were given out on the other side's
        // text: a structural edit says where their objects are on this
        // side, and one it made has no place here. A change of the text
        // says nothing, so they are found by bytes and key.
        for t in &next.tracked {
            if tracked.iter().any(|old| old.handle == t.handle) {
                continue;
            }
            leads.push(match (&entry.change, &t.path) {
                (Some(change), Some(path)) if forward => Lead::Known(change.apply(path)),
                (Some(change), Some(path)) => Lead::Known(
                    next.nodes
                        .iter()
                        .flatten()
                        .find(|n| change.apply(&n.path).as_ref() == Some(path))
                        .map(|n| n.path.clone()),
                ),
                (Some(_), None) => Lead::Known(None),
                (None, _) => Lead::Search,
            });
            tracked.push(t.clone());
        }
        next.tracked = tracked;
        place(&mut next, leads);
        self.store.write(&path, &text)?;
        *self.doc(&path)? = next;
        Ok(())
    }

    pub fn history(&self) -> (Vec<EntrySummary>, Vec<EntrySummary>) {
        self.history.list()
    }

    /// Sets the selection; every address must name an object.
    pub fn select(&mut self, path: &str, addresses: &[Address]) -> Result<Vec<String>, Error> {
        let path = &self.canonical(path)?;
        let mut handles = Vec::new();
        for address in addresses {
            handles.push(self.resolve(path, address)?.0);
        }
        self.selection = handles
            .iter()
            .map(|h| (path.to_owned(), h.clone()))
            .collect();
        Ok(handles)
    }

    /// The selection as (document, handle), dropping objects that are gone.
    pub fn selection(&mut self) -> Vec<(String, String)> {
        let selection = std::mem::take(&mut self.selection);
        let kept: Vec<_> = selection
            .into_iter()
            .filter(|(doc, handle)| self.resolve(doc, &Address::Node(handle.clone())).is_ok())
            .collect();
        self.selection = kept.clone();
        kept
    }
}

/// Points every handle of `doc` at its object in the new text.
fn remap(doc: &mut Document, change: Option<&PathChange>) {
    let leads = doc
        .tracked
        .iter()
        .map(|t| match (change, &t.path) {
            // A path worked out from the edit is trusted only when the
            // object there is still of its type.
            (Some(change), Some(path)) => Lead::Known(change.apply(path)),
            _ => Lead::Search,
        })
        .collect();
    place(doc, leads);
}

/// What is known of where a handle's object is in the new text.
enum Lead {
    /// Here, or gone (`None`), unless no object of its type is there.
    Known(Option<NodePath>),
    /// Unknown: found by its bytes, then by its key.
    Search,
}

/// Points each handle at an object, one handle to an object: a handle whose
/// object went must not be found again in a sibling that has a handle of
/// its own. Known places go first, then bytes the changes carried, and a
/// key only names an object no other handle took.
fn place(doc: &mut Document, mut leads: Vec<Lead>) {
    let Some(nodes) = &doc.nodes else {
        // Waits for a text that parses; the last paths stay as they were.
        return;
    };
    let mut taken = vec![false; nodes.len()];
    let mut placed: Vec<Option<Option<NodePath>>> = vec![None; doc.tracked.len()];
    for (i, t) in doc.tracked.iter().enumerate() {
        match &leads[i] {
            Lead::Known(None) => placed[i] = Some(None),
            Lead::Known(Some(path)) => {
                match nodes
                    .iter()
                    .position(|n| n.path == *path && n.ty == t.ty)
                    .filter(|&at| !taken[at])
                {
                    Some(at) => {
                        taken[at] = true;
                        placed[i] = Some(Some(path.clone()));
                    }
                    None => leads[i] = Lead::Search,
                }
            }
            Lead::Search => {}
        }
    }
    for (i, t) in doc.tracked.iter().enumerate() {
        if placed[i].is_some() {
            continue;
        }
        let Some((start, end)) = t.span else { continue };
        if let Some(at) = nodes
            .iter()
            .position(|n| n.start == start && n.end == end && n.ty == t.ty)
            .filter(|&at| !taken[at])
        {
            taken[at] = true;
            placed[i] = Some(Some(nodes[at].path.clone()));
        }
    }
    for (i, t) in doc.tracked.iter().enumerate() {
        if placed[i].is_some() {
            continue;
        }
        let found = find_by_key(nodes, &t.key)
            .and_then(|n| nodes.iter().position(|m| m.path == n.path))
            .filter(|&at| !taken[at]);
        placed[i] = Some(found.map(|at| {
            taken[at] = true;
            nodes[at].path.clone()
        }));
    }
    for (t, path) in doc.tracked.iter_mut().zip(placed) {
        t.path = path.flatten();
        refresh(t, nodes);
    }
}

/// Takes a found object's key, type and bytes as they are now.
fn refresh(t: &mut Tracked, nodes: &[Node]) {
    match t
        .path
        .as_ref()
        .and_then(|p| nodes.iter().find(|n| n.path == *p))
    {
        Some(node) => {
            t.key = node.key.clone();
            t.ty = node.ty.clone();
            t.span = Some((node.start, node.end));
        }
        None => t.span = None,
    }
}

/// The one change that turns `before` into `after`: what lies between their
/// common start and common end.
///
/// Text added or taken out between equal neighbours could be read at more
/// than one place (`A\nA\n` + `A\n`): it is read as whole lines where it
/// can be, as a person typing a line wrote it. Handles follow objects by
/// these changes, so the reading decides which of two alike objects is the
/// new one.
pub fn diff(before: &str, after: &str) -> Vec<Change> {
    if before == after {
        return Vec::new();
    }
    let (b, a) = (before.as_bytes(), after.as_bytes());
    let mut start = b.iter().zip(a).take_while(|(x, y)| x == y).count();
    while !before.is_char_boundary(start) || !after.is_char_boundary(start) {
        start -= 1;
    }
    let common_tail = |limit: usize| {
        b.iter()
            .rev()
            .zip(a.iter().rev())
            .take(limit)
            .take_while(|(x, y)| x == y)
            .count()
    };
    let shorter = b.len().min(a.len());
    let mut tail = common_tail(shorter - start);
    while !before.is_char_boundary(b.len() - tail) || !after.is_char_boundary(a.len() - tail) {
        tail -= 1;
    }
    if start + tail == shorter {
        // Only added or only taken out: it may sit anywhere from where the
        // common end could begin to where the common start stops.
        let (short, long) = if b.len() < a.len() {
            (before, after)
        } else {
            (after, before)
        };
        let width = long.len() - short.len();
        let earliest = shorter - common_tail(shorter);
        let line_start = (earliest..=start).find(|&k| {
            (k == 0 || short.as_bytes()[k - 1] == b'\n')
                && short.is_char_boundary(k)
                && long.is_char_boundary(k + width)
        });
        if let Some(k) = line_start {
            return vec![if b.len() < a.len() {
                Change {
                    range: k..k,
                    replacement: after[k..k + width].to_owned(),
                }
            } else {
                Change {
                    range: k..k + width,
                    replacement: String::new(),
                }
            }];
        }
    }
    vec![Change {
        range: start..b.len() - tail,
        replacement: after[start..a.len() - tail].to_owned(),
    }]
}

/// `text` (an object) with `id: <name>` as its first member, unless it has
/// an `id`; the name is the type's, lowercased, numbered past `taken`.
fn with_id(text: &str, taken: &[String]) -> Result<String, Error> {
    let nodes = outline::outline(text)
        .ok_or_else(|| Error::Invalid("the text to insert is not one QML object".to_owned()))?;
    let root = &nodes[0];
    if root.id.is_some() {
        return Ok(text.to_owned());
    }
    let ty = root.ty.rsplit('.').next().unwrap_or(&root.ty);
    let mut stem: String = ty.chars().take(1).flat_map(char::to_lowercase).collect();
    stem.push_str(&ty[ty.chars().next().map_or(0, char::len_utf8)..]);
    let name = (1..)
        .map(|n| format!("{stem}{n}"))
        .find(|name| !taken.contains(name))
        .expect("an unused name");
    // The id goes first, where a reader looks for it: on the brace's line
    // for an object written on one line, else on a line of its own indented
    // like the first member.
    let open = text[root.start..]
        .find('{')
        .map(|i| root.start + i + 1)
        .ok_or_else(|| Error::Invalid("the text to insert is not one QML object".to_owned()))?;
    let rest = &text[open..root.end];
    let out = match rest.find('\n') {
        Some(newline) if rest[..newline].trim().is_empty() && rest[newline..].trim() != "}" => {
            let body = &rest[newline + 1..];
            let indent: String = body
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            format!("{}\n{indent}id: {name}{}", &text[..open], &text[open..])
        }
        _ if rest.trim() == "}" => format!("{} id: {name} }}{}", &text[..open], &text[root.end..]),
        _ => format!("{} id: {name};{}", &text[..open], &text[open..]),
    };
    if outline::outline(&out).is_none() {
        return Err(Error::Invalid(format!("an id cannot be given to `{text}`")));
    }
    Ok(out)
}

/// How a document imports a module.
enum Imported {
    /// Its types are written bare.
    Bare,
    /// Only under a name: its types are written `Name.Type`.
    As(String),
    No,
}

fn imported(text: &str, module: &str) -> Imported {
    let allocator = qml_syntax::Allocator::default();
    let parsed = qml_syntax::parse(&allocator, text);
    let mut out = Imported::No;
    for i in &parsed.document.imports {
        let named = match &i.source {
            qml_syntax::ast::QmlImportSource::Module(name) => name.to_string() == module,
            qml_syntax::ast::QmlImportSource::Path(path) => path.trim_matches('"') == module,
        };
        match (&i.alias, named) {
            (_, false) => {}
            (None, true) => return Imported::Bare,
            (Some(alias), true) => {
                if matches!(out, Imported::No) {
                    out = Imported::As(alias.name.to_string());
                }
            }
        }
    }
    out
}

/// `text` (an object) with its type written `qualifier.Type`, unless it is
/// written qualified already.
fn qualified(text: &str, qualifier: &str) -> Result<String, Error> {
    let nodes = outline::outline(text)
        .ok_or_else(|| Error::Invalid("the text to insert is not one QML object".to_owned()))?;
    let root = &nodes[0];
    if root.ty.contains('.') {
        return Ok(text.to_owned());
    }
    Ok(format!(
        "{}{qualifier}.{}",
        &text[..root.start],
        &text[root.start..]
    ))
}

/// `text` importing `module`, which it does not yet: a line after the last
/// import.
fn with_import(text: &str, module: &str) -> Result<String, Error> {
    let allocator = qml_syntax::Allocator::default();
    let parsed = qml_syntax::parse(&allocator, text);
    let imports = &parsed.document.imports;
    let line = if module.contains('/') || module.ends_with(".js") {
        format!("import \"{module}\"")
    } else {
        format!("import {module}")
    };
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let out = match imports.last() {
        Some(last) => {
            let end = last.span.end as usize;
            let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
            format!("{}{line}{nl}{}", &text[..line_end], &text[line_end..])
        }
        None => format!("{line}{nl}{nl}{text}"),
    };
    if outline::outline(&out).is_none() {
        return Err(Error::Invalid(format!(
            "`{module}` cannot be imported here"
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
