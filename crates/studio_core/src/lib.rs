//! Ljós Studio's session: the documents of an open project, the objects in
//! them under addresses that survive edits, the selection and the history.
//!
//! Every way of changing a design goes through [`Session`]: the canvas, the
//! code view and an agent's MCP call alike. So an agent's edit shows on the
//! canvas, a person can undo it, and two writers never silently overwrite
//! each other: an edit names the document generation it was made against,
//! and one made against an older generation is moved past what others
//! wrote since (by the handles it names, or by its byte ranges) or refused
//! as a conflict that says who changed what.
//!
//! The file is the document (DESIGN.md goal 1). Edits are `qml_edit`'s
//! lossless splices, history entries are the changed ranges and their
//! inverses, and undo gives back the bytes.

mod address;
pub mod flows;
mod history;
mod log;
pub mod models;
pub mod outline;
pub mod place;
pub mod preview;
pub mod review;
pub mod scene3d;
pub mod slots;
pub mod states;
mod store;
pub mod theme;

use std::collections::BTreeMap;

pub use address::{Address, PathChange, parse_path};
pub use flows::{Goal, Interaction};
pub use history::{Entry, EntrySummary};
pub use models::Model;
pub use outline::{Binding, DeclaredSignal, Import, Node, is_literal_text};
pub use qml_edit::{Change, Error as EditError, NodePath, Step};
pub use review::{Anchor, Comment, Note};
pub use slots::Mode;
pub use states::{ChangeInfo, StateEdit, StateInfo, TransitionInfo, Variants};
#[cfg(feature = "fs")]
pub use store::FsStore;
pub use store::{MemStore, Store, StoreError, document_path, inside, listed};

use address::{Tracked, find_by_key};
use history::{History, Replayed};
use log::{CHILDREN, Lease, Log, Logged, STATES, TAPS, Touch, reaches};
pub use log::{LEASE_LIFE_MS, LEASE_TTL_MS, LeaseView};

/// Who wrote a change found in the file: nobody Studio knows.
pub const OUTSIDE: &str = "outside Studio";

/// A block's line in its module's `qmldir`: what undo takes back and redo
/// puts back, around whatever else was written to the file since.
#[derive(Clone, Debug)]
pub(crate) struct Declared {
    qmldir: String,
    module: String,
    line: String,
}

impl Declared {
    fn of(path: &str) -> Result<Self, Error> {
        let (dir, file) = path.rsplit_once('/').ok_or_else(|| {
            Error::Invalid(format!(
                "a block is in a module's directory, and `{path}` is in none"
            ))
        })?;
        let name = file.strip_suffix(".qml").unwrap_or(file);
        if !name.starts_with(|c: char| c.is_ascii_uppercase())
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(Error::Invalid(format!(
                "`{name}` is no type's name: start it with a capital"
            )));
        }
        if !dir.split('/').all(|part| {
            part.starts_with(|c: char| c.is_ascii_alphabetic())
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }) {
            return Err(Error::Invalid(format!("`{dir}` is no module's name")));
        }
        Ok(Self {
            qmldir: format!("{dir}/qmldir"),
            module: format!("module {}", dir.replace('/', ".")),
            line: format!("# block {name} 1.0 {file}"),
        })
    }

    fn write(&self, store: &dyn Store) -> Result<(), Error> {
        let text = match store.exists(&self.qmldir) {
            true => store.read(&self.qmldir)?,
            false => format!("{}\n", self.module),
        };
        let gap = if text.is_empty() || text.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        store.write(&self.qmldir, &format!("{text}{gap}{}\n", self.line))?;
        Ok(())
    }

    /// A `qmldir` left with only the module line it would be made with goes,
    /// a module with nothing in it being nobody's.
    fn take_back(&self, store: &dyn Store) -> Result<(), Error> {
        if !store.exists(&self.qmldir) {
            return Ok(());
        }
        let text = store.read(&self.qmldir)?;
        let kept: Vec<&str> = text.lines().filter(|l| *l != self.line).collect();
        if kept == [self.module.as_str()] {
            store.remove(&self.qmldir)?;
        } else {
            store.write(&self.qmldir, &format!("{}\n", kept.join("\n")))?;
        }
        Ok(())
    }
}

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
    #[error("no document `{0}` beside this one")]
    NoDocument(String),
    /// Only these open a document in place: a flow names one by `id`.
    #[error("{0} is not a `Loader` or a `StackView`")]
    NotANavigator(String),
    #[error("no flow Studio wrote on {0}")]
    NoInteraction(String),
    #[error("no comment `{0}`")]
    NoComment(String),
    /// Writing would lose the comments the sidecar holds.
    #[error("`.studio/review.json` does not read ({0}): mend it or delete it")]
    ReviewUnreadable(String),
    #[error(
        "`.studio/review.json` was changed by somebody else while it was open; it is read again: \
         look at the comments and try again"
    )]
    ReviewChanged,
    /// Another writer changed what this edit changes after the generation
    /// it was made at. Boxed: its seven fields would make every `Result`
    /// of the session carry them.
    #[error("{0}")]
    Conflict(Box<Conflict>),
    /// `handle` and all it holds are leased to `holder` until `until_ms`
    /// on the session's clock.
    #[error(
        "{handle} in `{doc}` is held by {holder} for a few seconds more: work elsewhere, or try \
         again once they let go"
    )]
    Leased {
        doc: String,
        handle: String,
        holder: String,
        until_ms: u64,
    },
    #[error(
        "{holder} has held objects for five minutes, the longest a lease lives: release them \
         (`node_select` with no nodes) and plan again"
    )]
    LeaseSpent { holder: String },
}

/// What an `Error::Conflict` names: `handle`'s binding `name`, or the
/// object itself when `name` is `None`, changed by `author` at
/// `generation` (`label`). `now` is the binding's value now, `None` when it
/// or its object is gone.
#[derive(Debug, thiserror::Error)]
#[error(
    "{author} changed {what} in `{doc}` at generation {generation} ({label}) after the \
     generation this edit was made at: read it again and decide",
    what = match (name, handle) {
        (Some(n), Some(h)) => format!("`{n}` of {h}"),
        (_, Some(h)) => h.clone(),
        (_, None) => "the same text".to_owned(),
    }
)]
pub struct Conflict {
    pub doc: String,
    /// `None` for a change of the text that is no one object's.
    pub handle: Option<String>,
    pub name: Option<String>,
    pub author: String,
    pub label: String,
    pub generation: u64,
    pub now: Option<String>,
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
    /// A flow: when `trigger` sends `signal` (a `TapHandler`'s tap when
    /// `None`), go to `goal` ([`flows`]).
    Interaction {
        trigger: Address,
        signal: Option<String>,
        goal: Goal,
    },
    /// Take out the flow `Interaction` wrote on `trigger` for `signal`.
    RemoveInteraction {
        trigger: Address,
        signal: Option<String>,
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
            Op::Interaction { trigger, goal, .. } => match goal {
                Goal::State { owner, .. } => std::iter::once(trigger).chain(owner).collect(),
                Goal::Open { navigator, .. } => vec![trigger, navigator],
            },
            Op::RemoveInteraction { trigger, .. } => vec![trigger],
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
            Op::Interaction { signal, .. } => {
                format!("{} interaction", signal.as_deref().unwrap_or("tapped"))
            }
            Op::RemoveInteraction { signal, .. } => {
                format!(
                    "remove {} interaction",
                    signal.as_deref().unwrap_or("tapped")
                )
            }
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
    /// The generation the edit was made against, when the document had
    /// moved on since and the edit was moved past what was written.
    pub rebased: Option<u64>,
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

/// A pause this long ends a step of typing.
const TYPING_PAUSE: std::time::Duration = std::time::Duration::from_secs(1);
/// The most one character takes.
const MAX_CHAR_BYTES: usize = 4;

/// The step the last character typed went into.
#[derive(Debug)]
struct Typed {
    doc: String,
    entry: u64,
    at: std::time::Duration,
    ended_word: bool,
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
    typed: Option<Typed>,
    /// Told the path of every file the session wrote or removed, after it
    /// did: what shows the project (a preview) follows edits through it
    /// rather than by watching the files.
    on_write: Vec<OnWrite>,
    /// The review comments, read from the sidecar when first needed.
    review: Option<review::Review>,
    /// Documents an undo of their making took away, with the comments that
    /// were on their objects then: a redo makes the document again, and
    /// they go back on their objects.
    unmade: BTreeMap<String, Vec<String>>,
    /// What was written to each held document lately, for rebasing an
    /// edit made against an older generation.
    logs: BTreeMap<String, Log>,
    leases: Vec<Lease>,
    /// Holders whose leases ran their whole life: they release before they
    /// hold again.
    spent: Vec<String>,
    /// Who is calling, and when, as [`Session::act_as`] last said.
    author: String,
    now_ms: u64,
}

/// A comment with the handle of its object, when it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentView {
    pub comment: Comment,
    pub node: Option<String>,
}

/// The author of the window's own gestures.
pub const PERSON: &str = "person";

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
            typed: None,
            on_write: Vec::new(),
            review: None,
            unmade: BTreeMap::new(),
            logs: BTreeMap::new(),
            leases: Vec::new(),
            spent: Vec::new(),
            author: PERSON.to_owned(),
            now_ms: 0,
        }
    }

    /// Says who makes the calls that follow and when (milliseconds on a
    /// clock that only goes forward). Their edits are theirs in the history
    /// and the log, their leases are renewed, and others' leases that
    /// lapsed by `now_ms` are let go: nothing watches the clock (rule 6),
    /// so a lease ends when somebody next calls.
    pub fn act_as(&mut self, author: &str, now_ms: u64) {
        self.tick(now_ms);
        let now = self.now_ms;
        author.clone_into(&mut self.author);
        for lease in self.leases.iter_mut().filter(|l| l.holder == author) {
            lease.renew(now);
        }
    }

    /// Moves the session's clock to `now_ms` (never back), and lets the
    /// leases that ran out by then go: what a window reads without calling.
    pub fn tick(&mut self, now_ms: u64) {
        self.now_ms = self.now_ms.max(now_ms);
        let now = self.now_ms;
        for lease in self.leases.iter().filter(|l| l.until_ms <= now) {
            if lease.spent() && !self.spent.contains(&lease.holder) {
                self.spent.push(lease.holder.clone());
            }
        }
        self.leases.retain(|l| l.until_ms > now);
    }

    /// The session's clock, as [`Session::act_as`] and [`Session::tick`]
    /// last set it.
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Who [`Session::act_as`] last said is calling.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Calls `f` with the path of each file the session writes or removes
    /// from now on, after the store has it.
    pub fn on_write(&mut self, f: impl FnMut(&str) + Send + 'static) {
        self.on_write.push(Box::new(f));
    }

    /// Calls `f` with each file the session is about to write (with its
    /// text) or remove (`None`) from now on: told before the store has it,
    /// so whoever watches the disk knows the change for its own however
    /// fast the disk tells them.
    pub fn on_writing(&mut self, f: impl Fn(&str, Option<&str>) + Send + Sync + 'static) {
        let inner = std::mem::replace(&mut self.store, Box::new(MemStore::default()));
        self.store = Box::new(store::Announcing {
            inner,
            writing: Box::new(f),
        });
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

    /// Whether the project holds no file at all that a walk would list.
    pub fn blank(&self) -> bool {
        self.store.list("").is_empty()
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
        // The log's offsets and the leases' handles are of the text that
        // went with it.
        self.logs.remove(path);
        self.leases.retain(|l| l.doc != path);
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
                        head: None,
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

    /// Where `node` is declared in `path`: its line and column, one-based, as
    /// the running preview names an object.
    ///
    /// # Errors
    /// A document that is not held.
    pub fn declared_where(&mut self, path: &str, node: &Node) -> Result<(usize, usize), Error> {
        let text = &self.doc(path)?.text;
        let start = u32::try_from(node.start).unwrap_or(u32::MAX);
        Ok(qml_syntax::position(text, start))
    }

    /// The object declared at `line` and `column` (one-based; `column` counted
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

    /// Whether an edit made against `given` can be applied now: `None` when
    /// it was made against the current generation (or names none), the
    /// generation it was made against when it is moved past what others
    /// wrote since. Only an edit whose objects are named by handle or `id`
    /// is moved; one by path is stale, as is one older than the log or past
    /// a change found in the file, whose ranges are a guess.
    fn rebase(&mut self, path: &str, given: Option<u64>, op: &Op) -> Result<Option<u64>, Error> {
        self.sync(path)?;
        let current = self.doc(path)?.generation;
        let Some(given) = given.filter(|g| *g != current) else {
            return Ok(None);
        };
        let stale = || Error::Stale {
            doc: path.to_owned(),
            given,
            current,
        };
        if op.addresses().iter().any(|a| matches!(a, Address::Path(_))) {
            return Err(stale());
        }
        let author = self.author.clone();
        let others: Vec<Logged> = self
            .logs
            .get(path)
            .and_then(|log| log.since(given, current))
            .ok_or_else(stale)?
            .into_iter()
            .filter(|e| e.author != author)
            .cloned()
            .collect();
        if others.iter().any(|e| e.outside || e.touched.is_none()) {
            return Err(stale());
        }
        let intent = self.intent(path, op)?;
        // A child index counts the children of the generation it was read
        // at; an append has none to count.
        let counted = matches!(
            op,
            Op::Insert { index: Some(_), .. } | Op::Move { index: Some(_), .. }
        );
        let doc = self.doc(path)?;
        // The newest change names what is there now.
        for entry in others.iter().rev() {
            for (h, name) in &intent {
                let hit = entry.touched.iter().flatten().find(|(a, m)| {
                    (m.is_none() && within(doc, *h, *a))
                        || (name.is_none() && within(doc, *a, *h))
                        // Two writers' children of one parent both land,
                        // unless one of them says where among the others.
                        || (a == h && m == name && (counted || name.as_deref() != Some(CHILDREN)))
                });
                if hit.is_some() {
                    let now = name.as_ref().and_then(|n| {
                        let at = doc.tracked.iter().find(|t| t.handle == *h)?.path.as_ref()?;
                        let node = doc.nodes.as_ref()?.iter().find(|x| x.path == *at)?;
                        Some(node.bindings.iter().find(|b| b.name == *n)?.value.clone())
                    });
                    return Err(Error::Conflict(Box::new(Conflict {
                        doc: path.to_owned(),
                        handle: Some(format!("n{h}")),
                        name: name.clone(),
                        author: entry.author.clone(),
                        label: entry.label.clone(),
                        generation: entry.generation,
                        now,
                    })));
                }
            }
        }
        Ok(Some(given))
    }

    /// What `op` means to change, by the handles its addresses name, which
    /// need not be found in the text any more: an object another writer
    /// took out is still the one this edit meant.
    fn intent(&mut self, path: &str, op: &Op) -> Result<Vec<Touch>, Error> {
        let handle = |s: &mut Self, address: &Address| -> Result<u64, Error> {
            let not_found = |a: String| Error::NotFound {
                doc: path.to_owned(),
                address: a,
            };
            match address {
                Address::Node(h) => h
                    .strip_prefix('n')
                    .and_then(|n| n.parse().ok())
                    .ok_or_else(|| not_found(h.clone())),
                _ => Ok(number(&s.resolve_now(path, address)?.0)),
            }
        };
        Ok(match op {
            Op::Insert { parent, .. } | Op::Contribute { parent, .. } => {
                vec![(handle(self, parent)?, Some(CHILDREN.to_owned()))]
            }
            Op::Move { node, parent, .. } => vec![
                (handle(self, node)?, None),
                (handle(self, parent)?, Some(CHILDREN.to_owned())),
            ],
            Op::Remove { node } | Op::RemoveContribution { node } => {
                vec![(handle(self, node)?, None)]
            }
            Op::SetBinding { node, name, .. } | Op::RemoveBinding { node, name } => {
                let h = handle(self, node)?;
                // Writing the binding replaces or takes out what is written
                // in its value, as a remove of those objects would.
                let mut touched = vec![(h, Some(name.clone()))];
                touched.extend(in_binding(self.doc(path)?, h, name).map(|t| (t, None)));
                touched
            }
            Op::State { node, .. } => vec![(handle(self, node)?, Some(STATES.to_owned()))],
            // Not planned: planning reads the text as it is now, and would
            // answer for what another writer did instead of naming them.
            Op::Interaction {
                trigger, signal, ..
            }
            | Op::RemoveInteraction { trigger, signal } => {
                let h = handle(self, trigger)?;
                let Some(signal) = signal else {
                    // Taking a tap flow out changes its `TapHandler`, however
                    // little else that holds. What adds one, or a handler
                    // with no flow in it now, meets only the `onTapped` that
                    // is or was written there: another writer may have just
                    // taken the flow out of it.
                    let removes = matches!(op, Op::RemoveInteraction { .. });
                    let mut touched = vec![(h, Some(TAPS.to_owned()))];
                    touched.extend(self.taps_of(path, h)?.into_iter().map(|(tap, tapped)| {
                        (tap, (!removes || !tapped).then(|| "onTapped".to_owned()))
                    }));
                    return Ok(touched);
                };
                vec![(h, Some(flows::handler_name(signal)))]
            }
        })
    }

    /// The edit a flow is written as, the handler or `TapHandler` that
    /// another writer's edit can meet; `None` for any other op.
    fn flow_edit(&mut self, path: &str, op: &Op) -> Result<Option<Op>, Error> {
        Ok(match op {
            Op::Interaction {
                trigger,
                signal,
                goal,
            } => Some(self.plan_interaction(path, trigger, signal.as_deref(), goal)?),
            Op::RemoveInteraction { trigger, signal } => {
                Some(self.plan_remove_interaction(path, trigger, signal.as_deref())?)
            }
            _ => None,
        })
    }

    /// What `op` changes in the text it is about to be applied to: its
    /// intent, with an object it replaces, moves or takes out standing for
    /// all it holds, and a move or remove taking a child from its parent too.
    fn touches(&mut self, path: &str, op: &Op) -> Result<Vec<Touch>, Error> {
        if let Some(edit) = self.flow_edit(path, op)? {
            return self.touches(path, &edit);
        }
        let intent = self.intent(path, op)?;
        let doc = self.doc(path)?;
        let mut out = Vec::with_capacity(intent.len());
        for (h, name) in intent {
            if name.is_none() {
                let at = doc
                    .tracked
                    .iter()
                    .find(|t| t.handle == h)
                    .and_then(|t| t.path.clone());
                let leaves_parent = matches!(
                    op,
                    Op::Move { .. } | Op::Remove { .. } | Op::RemoveContribution { .. }
                );
                if let (true, Some(at)) = (leaves_parent, &at)
                    && let Some(parent) = at.0.split_last().map(|(_, p)| p)
                    && let Some(t) = doc
                        .tracked
                        .iter()
                        .find(|t| t.path.as_ref().is_some_and(|p| p.0 == parent))
                {
                    out.push((t.handle, Some(CHILDREN.to_owned())));
                }
                out.extend(
                    doc.tracked
                        .iter()
                        .filter(|t| t.handle != h && within(doc, t.handle, h))
                        .map(|t| (t.handle, None)),
                );
            }
            out.push((h, name));
        }
        Ok(out)
    }

    /// What a change of the text touches, on the text before it: in each
    /// innermost object a range reaches, the binding holding it, or the
    /// object itself when no binding does. `None` when the text does not
    /// parse and names no objects.
    fn text_touches(&mut self, path: &str, changes: &[Change]) -> Option<Vec<Touch>> {
        let (tree, _) = self.tree_now(path).ok()?;
        let mut out: Vec<Touch> = Vec::new();
        for change in changes {
            let r = &change.range;
            let holds = |n: &Node| {
                n.start <= r.start
                    && r.end <= n.end
                    && !(r.is_empty() && (r.start == n.start || r.start == n.end))
            };
            for (handle, node) in &tree {
                if !(reaches(r, (node.start, node.end)) || holds(node)) {
                    continue;
                }
                // A range taking in a whole child changes the parent's
                // children, as a remove of it does.
                if !r.is_empty()
                    && r.start <= node.start
                    && node.end <= r.end
                    && let Some((qml_edit::Step::Child(_), up)) = node.path.0.split_last()
                    && let Some((parent, _)) = tree.iter().find(|(_, m)| m.path.0 == up)
                {
                    let touch = (number(parent), Some(CHILDREN.to_owned()));
                    if !out.contains(&touch) {
                        out.push(touch);
                    }
                }
                let inner = tree.iter().any(|(_, m)| {
                    m.path.0.len() > node.path.0.len()
                        && m.path.0.starts_with(&node.path.0)
                        && holds(m)
                });
                if inner {
                    continue;
                }
                let name = node
                    .bindings
                    .iter()
                    .find(|b| b.start <= r.start && r.end <= b.end)
                    .map(|b| b.name.clone());
                let touch = (number(handle), name);
                if !out.contains(&touch) {
                    out.push(touch);
                }
            }
        }
        Some(out)
    }

    /// Refuses a write by the caller to objects another writer holds: any
    /// of `handles`, or anything they hold.
    fn check_leased(
        &self,
        path: &str,
        handles: impl IntoIterator<Item = u64>,
    ) -> Result<(), Error> {
        let Some(doc) = self.docs.get(path) else {
            return Ok(());
        };
        for h in handles {
            if let Some(lease) = self.others_leases(path).find(|l| within(doc, h, l.handle)) {
                return Err(self.leased(lease));
            }
        }
        Ok(())
    }

    /// Refuses a change of the text by the caller that reaches into an
    /// object another writer holds.
    fn check_leased_ranges(&self, path: &str, changes: &[Change]) -> Result<(), Error> {
        let Some(doc) = self.docs.get(path) else {
            return Ok(());
        };
        for lease in self.others_leases(path) {
            let span = doc
                .tracked
                .iter()
                .find(|t| t.handle == lease.handle)
                .and_then(|t| t.span);
            if let Some(span) = span
                && changes.iter().any(|c| reaches(&c.range, span))
            {
                return Err(self.leased(lease));
            }
        }
        Ok(())
    }

    fn others_leases<'s>(&'s self, path: &'s str) -> impl Iterator<Item = &'s Lease> + 's {
        self.leases
            .iter()
            .filter(move |l| l.doc == path && l.holder != self.author && l.until_ms > self.now_ms)
    }

    fn leased(&self, lease: &Lease) -> Error {
        Error::Leased {
            doc: lease.doc.clone(),
            handle: format!("n{}", lease.handle),
            holder: lease.holder.clone(),
            until_ms: lease.until_ms,
        }
    }

    /// Applies a structural edit. An edit that names an object by path needs
    /// the `generation` the path was read at: a path is true of one text,
    /// and the only address that would name another object without notice.
    ///
    /// An edit made against an older generation that names its objects by
    /// handle or `id` is applied to the text as it is now, unless another
    /// writer changed what it changes in between ([`Error::Conflict`]).
    ///
    /// # Errors
    /// A path without a `generation`, a stale `generation` the log cannot
    /// rebase from, a conflict, an object another writer holds, an address
    /// that names nothing, a document that does not parse, or the edit's own
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
        let rebased = self.rebase(path, generation, op)?;
        let before = self.doc(path)?.text.clone();
        let (text, change, focus) = self.plan(path, &before, op)?;
        let changes = diff(&before, &text);
        let touched = self.touches(path, op)?;
        self.check_leased(path, touched.iter().map(|(h, _)| *h))?;
        if dry_run {
            return Ok(Outcome {
                doc: path.to_owned(),
                generation: self.doc(path)?.generation,
                unchanged: text == before,
                changes,
                history: None,
                node: None,
                text: Some(text),
                rebased,
            });
        }
        if text == before {
            let node = match focus {
                Some(at) => Some(self.handle_at(path, &at)?),
                None => None,
            };
            return self.unchanged(path, node);
        }
        let label = match op {
            Op::Interaction {
                trigger,
                signal,
                goal,
            } => self.interaction_label(path, trigger, signal.as_deref(), goal),
            _ => op.label(),
        };
        let entry = self.commit(path, before, text, Some(change), label, Some(touched))?;
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
            rebased,
        })
    }

    /// Applies several structural edits as one: one write, so one reload, and
    /// one history entry, so one undo. Later edits see what earlier ones did;
    /// the handles of the objects they put or moved are answered in order.
    ///
    /// # Errors
    /// As [`edit`](Self::edit), for any of them; nothing changes then.
    pub fn edit_many(
        &mut self,
        path: &str,
        generation: Option<u64>,
        ops: &[Op],
        label: &str,
        dry_run: bool,
    ) -> Result<(Outcome, Vec<String>), Error> {
        let path = &self.canonical(path)?;
        if generation.is_none()
            && let Some(Address::Path(p)) = ops
                .iter()
                .flat_map(Op::addresses)
                .find(|a| matches!(a, Address::Path(_)))
        {
            return Err(Error::Invalid(format!(
                "the path `{p}` names an object only in the generation it was read at: give \
                 `generation`"
            )));
        }
        // Against the log as it stands: what an older read could not have seen
        // is judged before any of these edits is part of it.
        let mut rebased = None;
        for op in ops {
            rebased = self.rebase(path, generation, op)?.or(rebased);
        }
        // Comments follow handles: theirs must be known before the handles
        // move.
        self.look_up(path);
        let taps = self.taps_now(path);
        let saved = self.doc(path)?.clone();
        let mut nodes = Vec::new();
        let mut touched = Vec::new();
        for op in ops {
            if let Err(e) = self.apply_unwritten(path, op, &mut touched, &mut nodes) {
                *self.doc(path)? = saved;
                return Err(e);
            }
        }
        if let Err(e) = self.check_leased(path, touched.iter().map(|(h, _)| *h)) {
            *self.doc(path)? = saved;
            return Err(e);
        }
        let after = self.doc(path)?.text.clone();
        let before = saved.text.clone();
        let changes = diff(&before, &after);
        if dry_run {
            *self.doc(path)? = saved;
            return Ok((
                Outcome {
                    doc: path.to_owned(),
                    generation: self.doc(path)?.generation,
                    unchanged: after == before,
                    changes,
                    history: None,
                    node: None,
                    text: Some(after),
                    rebased,
                },
                Vec::new(),
            ));
        }
        if after == before {
            *self.doc(path)? = saved;
            let outcome = self.unchanged(path, None)?;
            return Ok((outcome, nodes));
        }
        if let Err(e) = self.store.write(path, &after) {
            *self.doc(path)? = saved;
            return Err(e.into());
        }
        // One edit is one generation, however many steps it took: the log
        // holds an entry for every generation or rebases across none.
        let generation = saved.generation + 1;
        let doc = self.doc(path)?;
        doc.generation = generation;
        let handles_after = doc.tracked.clone();
        let entry = Entry {
            id: 0,
            doc: path.to_owned(),
            label: label.to_owned(),
            author: self.author.clone(),
            outside: false,
            generation,
            side: side(&after),
            forward: changes.clone(),
            inverse: diff(&after, &before),
            created: false,
            group: None,
            declares: None,
            handles_before: saved.tracked,
            handles_after,
            change: None,
            exact: true,
        };
        let touched = self.with_taps(path, &taps, Some(touched));
        self.log(&entry, touched);
        let id = self.history.push(entry);
        self.reanchor(path);
        self.wrote(path);
        Ok((
            Outcome {
                doc: path.to_owned(),
                generation,
                changes,
                history: Some(id),
                node: nodes.first().cloned(),
                text: None,
                unchanged: false,
                rebased,
            },
            nodes,
        ))
    }

    /// One edit of [`edit_many`](Self::edit_many), on the held text alone.
    fn apply_unwritten(
        &mut self,
        path: &str,
        op: &Op,
        touched: &mut Vec<Touch>,
        nodes: &mut Vec<String>,
    ) -> Result<(), Error> {
        for touch in self.touches(path, op)? {
            if !touched.contains(&touch) {
                touched.push(touch);
            }
        }
        let current = self.doc(path)?.text.clone();
        let (text, change, focus) = self.plan(path, &current, op)?;
        if text != current {
            let (next, _) = self.step(path, current, text, Some(change), String::new(), false)?;
            *self.doc(path)? = next;
        }
        if let Some(at) = focus {
            nodes.push(self.handle_at(path, &at)?);
        }
        Ok(())
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
                if let Some(name) = member_name(text) {
                    if index.is_some() || import.is_some() {
                        return Err(Error::Invalid(
                            "a member goes after the object's others, and imports nothing: \
                             leave `index` and `import` out"
                                .to_owned(),
                        ));
                    }
                    if has_member(&parent_node, name) {
                        return Err(Error::Invalid(format!(
                            "{} already has a member `{name}`",
                            parent_node.ty
                        )));
                    }
                    let out = qml_edit::apply(
                        before,
                        &qml_edit::Edit::InsertObject {
                            parent: parent_node.path.clone(),
                            index: count,
                            text: text.trim().to_owned(),
                        },
                    )
                    .map_err(|e| Error::Invalid(format!("that member does not fit: {e}")))?
                    .text;
                    return Ok((out, PathChange::Same, Some(parent_node.path.clone())));
                }
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
                    if let Imported::No = imported(&out, module) {
                        out = with_import(&out, module)?;
                    }
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
            Op::Interaction {
                trigger,
                signal,
                goal,
            } => {
                let edit = self.plan_interaction(path, trigger, signal.as_deref(), goal)?;
                self.plan(path, before, &edit)
            }
            Op::RemoveInteraction { trigger, signal } => {
                let edit = self.plan_remove_interaction(path, trigger, signal.as_deref())?;
                self.plan(path, before, &edit)
            }
        }
    }

    /// The code view's edit: replace `range` of the text (all of it when
    /// `None`) with `text`. The result need not parse; while it does not,
    /// structural edits wait and handles keep their objects for when it
    /// does.
    ///
    /// A `range` of an older generation is moved past what was written
    /// since. One that overlaps another's change is a conflict, and one that
    /// would leave a text that parses not parsing is refused: either alone
    /// was meant, the two together were not.
    pub fn replace_text(
        &mut self,
        path: &str,
        generation: Option<u64>,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        dry_run: bool,
    ) -> Result<Outcome, Error> {
        let path = &self.canonical(path)?;
        self.sync(path)?;
        let before = self.doc(path)?.text.clone();
        let current = self.doc(path)?.generation;
        let rebased = generation.filter(|g| *g != current);
        let range = match (rebased, range) {
            (None, range) => range.unwrap_or(0..before.len()),
            // All of an older text is not a range of this one.
            (Some(given), None) => {
                return Err(Error::Stale {
                    doc: path.to_owned(),
                    given,
                    current,
                });
            }
            (Some(given), Some(range)) => self.rebase_range(path, given, range, text)?,
        };
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
        if let Some(given) = rebased
            && self.doc(path)?.nodes.is_some()
            && outline::outline(&after).is_none()
        {
            return Err(Error::Invalid(format!(
                "moved past what was written to `{path}` since generation {given}, this edit \
                 would no longer parse: read the text again"
            )));
        }
        let changes = diff(&before, &after);
        self.check_leased_ranges(path, &changes)?;
        if dry_run {
            return Ok(Outcome {
                doc: path.to_owned(),
                generation: self.doc(path)?.generation,
                unchanged: after == before,
                changes,
                history: None,
                node: None,
                text: Some(after),
                rebased,
            });
        }
        if after == before {
            return self.unchanged(path, None);
        }
        let touched = self.text_touches(path, &changes);
        let entry = self.commit(path, before, after, None, "edit text".to_owned(), touched)?;
        Ok(Outcome {
            doc: path.to_owned(),
            generation: self.doc(path)?.generation,
            changes,
            history: Some(entry),
            node: None,
            text: None,
            unchanged: false,
            rebased,
        })
    }

    /// [`replace_text`](Self::replace_text) from a person typing. One
    /// character typed or removed joins the step before it, as a text editor
    /// does, unless a second has passed or it starts a new word; 26 characters
    /// are not 26 undos. A paste is a step of its own. `now` is any clock's.
    pub fn type_text(
        &mut self,
        path: &str,
        generation: u64,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        now: std::time::Duration,
    ) -> Result<Outcome, Error> {
        // The newest of anybody's, and this writer's: a word another writer
        // landed in the middle of is two words.
        let top = self
            .history
            .peek_undo(None)
            .filter(|e| e.author == self.author)
            .map(|e| e.id);
        let mut out = self.replace_text(path, Some(generation), range, text, false)?;
        let Some(entry) = out.history else {
            return Ok(out);
        };
        let [change] = out.changes.as_slice() else {
            self.typed = None;
            return Ok(out);
        };
        let typed = change.replacement.chars().next();
        if change.replacement.chars().count() > 1 || change.range.len() > MAX_CHAR_BYTES {
            self.typed = None;
            return Ok(out);
        }
        let joins = self.typed.as_ref().is_some_and(|last| {
            last.doc == out.doc
                && Some(last.entry) == top
                && now.saturating_sub(last.at) < TYPING_PAUSE
                && !(last.ended_word && typed.is_some_and(|c| !c.is_whitespace()))
        });
        if joins {
            out.history = Some(self.join_last(&out.doc)?);
        }
        self.typed = Some(Typed {
            doc: out.doc.clone(),
            entry: out.history.unwrap_or(entry),
            at: now,
            ended_word: typed.is_some_and(char::is_whitespace),
        });
        Ok(out)
    }

    /// Makes the last entry and the one before it, both of `path`, one.
    fn join_last(&mut self, path: &str) -> Result<u64, Error> {
        let after = self.doc(path)?.text.clone();
        let (last, earlier) = self.history.pop_and_top();
        let (Some(last), Some(earlier)) = (last, earlier) else {
            return Err(Error::NothingTo("join"));
        };
        let before_last = qml_edit::splice(&after, &last.inverse);
        let first = qml_edit::splice(&before_last, &earlier.inverse);
        earlier.forward = diff(&first, &after);
        earlier.inverse = diff(&after, &first);
        // Already every handle on the text this leaves.
        earlier.handles_after = last.handles_after;
        // What the document is now is what an undo checks it against, and
        // what the log has since then is what it moves past.
        earlier.generation = last.generation;
        earlier.side = last.side;
        Ok(earlier.id)
    }

    /// `range` of generation `given`, moved past every change since. One
    /// that overlaps another writer's change is a conflict naming it; one
    /// that overlaps the caller's own is stale, since the caller has only
    /// to read what it wrote.
    fn rebase_range(
        &mut self,
        path: &str,
        given: u64,
        range: std::ops::Range<usize>,
        text: &str,
    ) -> Result<std::ops::Range<usize>, Error> {
        let current = self.doc(path)?.generation;
        let stale = || Error::Stale {
            doc: path.to_owned(),
            given,
            current,
        };
        let since = self
            .logs
            .get(path)
            .and_then(|log| log.since(given, current))
            .ok_or_else(stale)?;
        let mut change = Change {
            range,
            replacement: text.to_owned(),
        };
        for entry in since {
            if entry.outside {
                return Err(stale());
            }
            change = match qml_edit::transform(&change, &entry.changes) {
                Ok(moved) => moved,
                Err(_) if entry.author == self.author => return Err(stale()),
                Err(_) => return Err(conflict(path, entry, None)),
            };
        }
        Ok(change.range)
    }

    /// A new document `new_path` holding `new_text`, and `doc`'s text replaced
    /// by `text`, as one undo step labelled `label`. Nothing stays of either
    /// if the other can't be written.
    pub fn create_with_edit(
        &mut self,
        new_path: &str,
        new_text: &str,
        doc: &str,
        text: &str,
        label: &str,
        dry_run: bool,
    ) -> Result<Outcome, Error> {
        let edit = self.replace_text(doc, None, None, text, true)?;
        let made = self.create(new_path, new_text, dry_run)?;
        if dry_run {
            return Ok(edit);
        }
        let outcome = match self.replace_text(doc, None, None, text, false) {
            Ok(outcome) => outcome,
            Err(e) => {
                let entry = self
                    .history
                    .peek_undo(None)
                    .filter(|e| e.doc == made.doc && e.created)
                    .cloned();
                if let Some(entry) = entry {
                    self.replay(&entry, false)?;
                    self.history.pop_and_top();
                    self.wrote(&entry.doc);
                }
                return Err(e);
            }
        };
        self.history.group_newest();
        for path in [&made.doc, &outcome.doc] {
            if let Some(entry) = self.history.latest_mut(path) {
                entry.label = label.to_owned();
            }
        }
        Ok(outcome)
    }

    /// A new document with `text`.
    /// Creates `path` as [`Session::create`] does and declares it a block in
    /// the `qmldir` beside it, as one undo step. The directory is the module
    /// of a `qmldir` that does not say, the file's name the block's.
    pub fn create_block(
        &mut self,
        path: &str,
        text: &str,
        dry_run: bool,
    ) -> Result<Outcome, Error> {
        let declared = Declared::of(path)?;
        let qmldir = match self.store.exists(&declared.qmldir) {
            true => Some(self.store.read(&declared.qmldir)?),
            false => None,
        };
        if qmldir
            .as_deref()
            .is_some_and(|t| t.lines().any(|l| l == declared.line))
        {
            return Err(Error::Invalid(format!(
                "`{}` already declares `{}`",
                declared.qmldir, declared.line
            )));
        }
        let outcome = self.create(path, text, dry_run)?;
        if dry_run {
            return Ok(outcome);
        }
        if let Err(e) = declared.write(self.store.as_ref()) {
            self.replay_new_away(&outcome.doc)?;
            return Err(e);
        }
        if let Some(entry) = self.history.latest_mut(&outcome.doc) {
            entry.declares = Some(declared);
            entry.label = "create block".to_owned();
        }
        Ok(outcome)
    }

    /// Takes back the document just created, whose entry is on top.
    fn replay_new_away(&mut self, doc: &str) -> Result<(), Error> {
        let entry = self
            .history
            .peek_undo(None)
            .filter(|e| e.doc == doc && e.created)
            .cloned();
        if let Some(entry) = entry {
            self.replay(&entry, false)?;
            self.history.pop_and_top();
            self.wrote(&entry.doc);
        }
        Ok(())
    }

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
                rebased: None,
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
            group: None,
            declares: None,
            handles_before: Vec::new(),
            handles_after: Vec::new(),
            change: None,
            author: self.author.clone(),
            outside: false,
            generation,
            side: side(text),
            exact: true,
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
            rebased: None,
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
            rebased: None,
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
        if exists {
            self.check_leased_ranges(file, &changes)?;
        }
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
                rebased: None,
            });
        }
        if unchanged {
            return self.unchanged(file, None);
        }
        let entry = if exists {
            // A theme names no objects: nothing is rebased past it by handle.
            self.commit(file, before, after, None, label, None)?
        } else {
            let past = self.retired.get(file).copied().unwrap_or(0);
            let doc = Document::new(after.clone(), past);
            self.store.write(file, &after)?;
            let generation = doc.generation;
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
                group: None,
                declares: None,
                handles_before: Vec::new(),
                handles_after: Vec::new(),
                change: None,
                author: self.author.clone(),
                outside: false,
                generation,
                side: side(&after),
                exact: true,
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
            rebased: None,
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
        touched: Option<Vec<Touch>>,
    ) -> Result<u64, Error> {
        // Comments follow handles: theirs must be known before the handles
        // move.
        self.look_up(path);
        let taps = self.taps_now(path);
        let (next, entry) = self.step(path, before, after.clone(), change, label, false)?;
        self.store.write(path, &after)?;
        *self.doc(path)? = next;
        let touched = self.with_taps(path, &taps, touched);
        self.log(&entry, touched);
        let id = self.history.push(entry);
        self.reanchor(path);
        self.wrote(path);
        Ok(id)
    }

    /// Takes `after` as the document's text, which the file already has:
    /// moves the handles and records the entry, a change found in the file.
    fn record(
        &mut self,
        path: &str,
        before: String,
        after: String,
        change: Option<PathChange>,
        label: String,
    ) -> Result<u64, Error> {
        self.look_up(path);
        let (next, entry) = self.step(path, before, after, change, label, true)?;
        *self.doc(path)? = next;
        self.log(&entry, None);
        let id = self.history.push(entry);
        self.reanchor(path);
        Ok(id)
    }

    /// `touched`, and the `<taps>` of each object whose `TapHandler`s are not
    /// `before`'s any more. Read from the text on both sides, as how a
    /// handler got there has too many forms to name.
    fn with_taps(
        &self,
        path: &str,
        before: &flows::Taps,
        touched: Option<Vec<Touch>>,
    ) -> Option<Vec<Touch>> {
        let after = self.taps_now(path);
        let mut touched = touched?;
        let differs = before
            .iter()
            .chain(&after)
            .filter(|(parent, _)| before.get(parent) != after.get(parent))
            .map(|(parent, _)| (*parent, Some(TAPS.to_owned())));
        for touch in differs {
            if !touched.contains(&touch) {
                touched.push(touch);
            }
        }
        Some(touched)
    }

    /// Puts a new history entry's edit in its document's log.
    fn log(&mut self, entry: &Entry, touched: Option<Vec<Touch>>) {
        self.logs
            .entry(entry.doc.clone())
            .or_default()
            .push(Logged {
                generation: entry.generation,
                author: entry.author.clone(),
                label: entry.label.clone(),
                changes: entry.forward.clone(),
                touched,
                outside: entry.outside,
                reverts: None,
            });
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
        outside: bool,
    ) -> Result<(Document, Entry), Error> {
        let author = if outside { OUTSIDE } else { &self.author }.to_owned();
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
            author,
            outside,
            generation: next.generation,
            side: side(&after),
            forward,
            inverse: diff(&after, &before),
            created: false,
            group: None,
            declares: None,
            handles_before,
            handles_after: next.tracked.clone(),
            change,
            exact: true,
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

    /// Undoes the caller's last edit, or a change found in the file, which
    /// is nobody's and so everybody's: its document gets the bytes it had
    /// before, moved past what others wrote since. The history moves only
    /// once the file has: one that could not be written can be tried again.
    ///
    /// # Errors
    /// Nothing to undo, a [`Error::Conflict`] with a later change of the
    /// same bytes, an object another writer holds, or the store's refusal.
    pub fn undo(&mut self) -> Result<EntrySummary, Error> {
        let author = self.author.clone();
        self.undo_by(Some(&author))
    }

    /// Undoes the newest edit, whoever made it.
    pub fn undo_newest(&mut self) -> Result<EntrySummary, Error> {
        self.undo_by(None)
    }

    fn undo_by(&mut self, author: Option<&str>) -> Result<EntrySummary, Error> {
        // A change made outside Studio goes on top here and is what is
        // undone: the entry below it has offsets on a text the file left.
        while let Some(top) = self.history.peek_undo(author) {
            let doc = top.doc.clone();
            let partner = top
                .group
                .and_then(|id| self.history.done_entry(id))
                .map(|e| e.doc.clone());
            match self.sync(&doc).and_then(|()| match &partner {
                Some(other) => self.sync(other).map_err(|e| match e {
                    Error::Store(StoreError::Missing(_)) => Error::Exists(other.clone()),
                    e => e,
                }),
                None => Ok(()),
            }) {
                Ok(()) => break,
                Err(Error::Store(StoreError::Missing(_))) => self.history.forget(&doc),
                Err(Error::Exists(gone)) => self.history.forget(&gone),
                Err(e) => return Err(e),
            }
        }
        let entry = self
            .history
            .peek_undo(author)
            .cloned()
            .ok_or(Error::NothingTo("undo"))?;
        let partner = entry
            .group
            .and_then(|id| self.history.done_entry(id))
            .cloned();
        if let Some(partner) = partner.as_ref().filter(|p| p.created) {
            // Before anything moves: this is the usual reason it can't.
            self.check_unmakeable(partner)?;
        }
        // It moves only once it has happened: a write that failed leaves it
        // to be tried again, and the lists still say where the file is.
        let (generation, side, rebased) = self.replay(&entry, false)?;
        self.history
            .commit_undo(entry.id, generation, side, rebased);
        self.wrote(&entry.doc);
        if let Some(partner) = partner {
            match self.replay(&partner, false) {
                Ok((generation, side, rebased)) => {
                    self.history
                        .commit_undo(partner.id, generation, side, rebased);
                    self.wrote(&partner.doc);
                }
                Err(e) => {
                    // One step happens whole, so the first goes back.
                    let moved = self
                        .history
                        .undone_entry(entry.id)
                        .cloned()
                        .expect("just undone");
                    let (generation, side, rebased) = self.replay(&moved, true)?;
                    self.history
                        .commit_redo(entry.id, generation, side, rebased);
                    self.wrote(&entry.doc);
                    return Err(e);
                }
            }
        }
        Ok(entry.summary())
    }

    /// Redoes what the caller last undid.
    pub fn redo(&mut self) -> Result<EntrySummary, Error> {
        let author = self.author.clone();
        self.redo_by(Some(&author))
    }

    /// Redoes what was last undone, whoever undid it.
    pub fn redo_newest(&mut self) -> Result<EntrySummary, Error> {
        self.redo_by(None)
    }

    fn redo_by(&mut self, author: Option<&str>) -> Result<EntrySummary, Error> {
        // A change found here ends what could be redone, like any edit.
        while let Some(top) = self.history.peek_redo(author) {
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
            .peek_redo(author)
            .cloned()
            .ok_or(Error::NothingTo("redo"))?;
        let partner = entry
            .group
            .and_then(|id| self.history.undone_entry(id))
            .cloned();
        if let Some(partner) = partner.as_ref().filter(|p| !p.created) {
            self.sync(&partner.doc)?;
        }
        let (generation, side, rebased) = self.replay(&entry, true)?;
        self.history
            .commit_redo(entry.id, generation, side, rebased);
        self.wrote(&entry.doc);
        if let Some(partner) = partner {
            match self.replay(&partner, true) {
                Ok((generation, side, rebased)) => {
                    self.history
                        .commit_redo(partner.id, generation, side, rebased);
                    self.wrote(&partner.doc);
                }
                Err(e) => {
                    // One step happens whole, so the first goes back.
                    let moved = self
                        .history
                        .done_entry(entry.id)
                        .cloned()
                        .expect("just redone");
                    let (generation, side, rebased) = self.replay(&moved, false)?;
                    self.history
                        .commit_undo(entry.id, generation, side, rebased);
                    self.wrote(&entry.doc);
                    return Err(e);
                }
            }
        }
        Ok(entry.summary())
    }

    /// Taking the document `entry` made away takes what others wrote with it.
    fn check_unmakeable(&mut self, entry: &Entry) -> Result<(), Error> {
        let path = &entry.doc;
        if side(&self.doc(path)?.text) == entry.side {
            return Ok(());
        }
        let latest = self.logs.get(path).and_then(|l| l.latest()).cloned();
        Err(match latest {
            Some(e) => conflict(path, &e, None),
            None => Error::Invalid(format!("`{path}` changed since it was made")),
        })
    }

    /// Brings the file and the document to that side of `entry`, for the
    /// caller to move the history and tell who watches: the generation and
    /// the hash of the text it left, and what the entry is now when it was
    /// moved past others' edits to get there.
    fn replay(
        &mut self,
        entry: &Entry,
        forward: bool,
    ) -> Result<(u64, u64, Option<Replayed>), Error> {
        let path = entry.doc.clone();
        if self.store.exists(&path) {
            self.look_up(&path);
        }
        if entry.created && !forward {
            let doc = self.doc(&path)?;
            let (generation, len) = (doc.generation, doc.text.len());
            self.check_unmakeable(entry)?;
            self.check_leased_ranges(
                &path,
                &[Change {
                    range: 0..len,
                    replacement: String::new(),
                }],
            )?;
            self.store.remove(&path)?;
            if let Some(declared) = &entry.declares {
                declared.take_back(self.store.as_ref())?;
            }
            self.retire(&path);
            let on_it: Vec<String> = self.review.as_ref().map_or_else(Vec::new, |r| {
                r.comments
                    .iter()
                    .filter(|c| c.doc == path && !c.orphaned)
                    .map(|c| c.id.clone())
                    .collect()
            });
            self.unmade.insert(path.clone(), on_it);
            self.reanchor(&path);
            return Ok((generation, side(""), None));
        }
        if entry.created && forward {
            let text = qml_edit::splice("", &entry.forward);
            let past = self.retired.get(&path).copied().unwrap_or(0);
            let mut doc = Document::new(text.clone(), past);
            doc.tracked = entry.handles_after.clone();
            let generation = doc.generation;
            self.store.write(&path, &text)?;
            if let Some(declared) = &entry.declares {
                declared.write(self.store.as_ref())?;
            }
            self.docs.insert(path.clone(), doc);
            self.remade(&path);
            return Ok((generation, side(&text), None));
        }
        let label = format!("{} {}", if forward { "redo" } else { "undo" }, entry.label);
        let changes = if forward {
            &entry.forward
        } else {
            &entry.inverse
        };
        let (current, generation) = {
            let doc = self.doc(&path)?;
            (doc.text.clone(), doc.generation)
        };
        if side(&current) == entry.side {
            self.check_leased_ranges(&path, changes)?;
            let reverts = Some(entry.generation);
            if entry.exact {
                let touched =
                    self.text_touches(&path, &diff(&current, &qml_edit::splice(&current, changes)));
                return self.replay_exact(entry, forward, label, touched, reverts);
            }
            let changes = changes.clone();
            return self.replay_text(&path, &current, &changes, label, forward, false, reverts);
        }
        // Others wrote since: the entry's changes are moved past theirs.
        let since: Vec<Logged> = self
            .logs
            .get(&path)
            .and_then(|l| l.since(entry.generation, generation))
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "too much was written to `{path}` since `{}` to move it past: do it by hand",
                    entry.label
                ))
            })?
            .into_iter()
            .cloned()
            .collect();
        let mut moved = changes.clone();
        for e in &since {
            if e.outside {
                return Err(conflict(&path, e, None));
            }
            moved = qml_edit::transform_all(&moved, &e.changes)
                .map_err(|_| conflict(&path, e, None))?;
        }
        self.check_leased_ranges(&path, &moved)?;
        self.replay_text(&path, &current, &moved, label, forward, true, None)
    }

    /// Applies an entry's changes, made on `current` or moved onto it, as a
    /// change of the text: handles are found again by bytes and key.
    #[allow(clippy::too_many_arguments)]
    fn replay_text(
        &mut self,
        path: &str,
        current: &str,
        changes: &[Change],
        label: String,
        forward: bool,
        rebased: bool,
        reverts: Option<u64>,
    ) -> Result<(u64, u64, Option<Replayed>), Error> {
        let text = qml_edit::splice(current, changes);
        if rebased && self.doc(path)?.nodes.is_some() && outline::outline(&text).is_none() {
            return Err(Error::Invalid(format!(
                "moved past what was written to `{path}` since, `{label}` would leave it not \
                 parsing: do it by hand"
            )));
        }
        let made = diff(current, &text);
        let back = diff(&text, current);
        let touched = self.text_touches(path, &made);
        let taps = self.taps_now(path);
        let doc = self.doc(path)?;
        let mut next = doc.clone();
        for change in &made {
            for t in &mut next.tracked {
                t.shift(change);
            }
        }
        next.set_text(text.clone());
        remap(&mut next, None);
        let generation = next.generation;
        self.store.write(path, &text)?;
        *self.doc(path)? = next;
        self.reanchor(path);
        let touched = self.with_taps(path, &taps, touched);
        self.log_replay(path, generation, label, made.clone(), touched, reverts);
        let replayed = if forward {
            Replayed {
                forward: made,
                inverse: back,
            }
        } else {
            Replayed {
                forward: back,
                inverse: made,
            }
        };
        Ok((generation, side(&text), Some(replayed)))
    }

    fn log_replay(
        &mut self,
        path: &str,
        generation: u64,
        label: String,
        changes: Vec<Change>,
        touched: Option<Vec<Touch>>,
        reverts: Option<u64>,
    ) {
        let author = self.author.clone();
        self.logs.entry(path.to_owned()).or_default().push(Logged {
            generation,
            author,
            label,
            changes,
            touched,
            outside: false,
            reverts,
        });
    }

    /// The entry's changes on the text it left, with the handles as they
    /// were on its other side.
    fn replay_exact(
        &mut self,
        entry: &Entry,
        forward: bool,
        label: String,
        touched: Option<Vec<Touch>>,
        reverts: Option<u64>,
    ) -> Result<(u64, u64, Option<Replayed>), Error> {
        let path = entry.doc.clone();
        let taps = self.taps_now(&path);
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
        let generation = next.generation;
        let made = diff(&doc.text, &text);
        self.store.write(&path, &text)?;
        *self.doc(&path)? = next;
        self.reanchor(&path);
        let touched = self.with_taps(&path, &taps, touched);
        self.log_replay(&path, generation, label, made, touched, reverts);
        Ok((generation, side(&text), None))
    }

    /// The comments an undo orphaned with `doc`, back on their objects
    /// now that a redo made it again: found by key, as in a new session,
    /// since the document's handles are new.
    fn remade(&mut self, doc: &str) {
        let Some(ids) = self.unmade.remove(doc) else {
            return;
        };
        let Some(review) = &mut self.review else {
            return;
        };
        for c in review.comments.iter_mut().filter(|c| ids.contains(&c.id)) {
            c.orphaned = false;
            review.bound.remove(&c.id);
        }
        review.looked_up.retain(|d| d != doc);
        self.look_up(doc);
        self.save_derived();
    }

    fn review(&mut self) -> &mut review::Review {
        if self.review.is_none() {
            self.review = Some(self.read_review());
        }
        self.review.as_mut().expect("just read")
    }

    /// The sidecar as it is now. Only a sidecar that is not there means no
    /// comments: one that would not read (not text, not to be opened) is
    /// unreadable, and never written over.
    fn read_review(&self) -> review::Review {
        match self.store.read(review::FILE) {
            Ok(text) => review::Review::read(Some(&text)),
            Err(StoreError::Missing(_)) => review::Review::read(None),
            Err(e) => review::Review::unreadable(e.to_string()),
        }
    }

    /// Reads the sidecar again where somebody else changed it since this
    /// session last read or wrote it, or where it did not read then (it
    /// may be mended now). Comments the session follows stay on their
    /// objects, and where that differs from the file it is written down;
    /// the rest are found by key, as in a new session.
    fn refresh_review(&mut self) {
        let Some(held) = &self.review else {
            return;
        };
        let now = self.read_review();
        if held.unreadable.is_none() && now.unreadable.is_none() && now.seen == held.seen {
            return;
        }
        let on_disk = now.comments.clone();
        let held = self.review.take().expect("just looked at");
        let merged = held.merged(now);
        let differs = merged.unreadable.is_none() && merged.comments != on_disk;
        self.review = Some(merged);
        if differs {
            self.save_derived();
        }
    }

    /// Writes down what follows from the design rather than what somebody
    /// asked for, so a sidecar changed meanwhile is no one's to be told of:
    /// the two are merged and it goes again. Whatever is still not written
    /// is in the session, for the next write.
    fn save_derived(&mut self) {
        if matches!(self.save_review(), Err(Error::ReviewChanged)) {
            let _ = self.save_review();
        }
    }

    /// Finds the objects of `doc`'s comments by their keys, once a session:
    /// from then on they follow handles.
    fn look_up(&mut self, doc: &str) {
        let review = self.review();
        if review.looked_up.iter().any(|d| d == doc)
            || !review.comments.iter().any(|c| c.doc == doc)
        {
            return;
        }
        if !self.store.exists(doc) {
            return;
        }
        // The text the handles are on now: the caller brought it level with
        // the file, or is bringing it (`record`), so this must not sync. A
        // text that does not parse, or a document not held, has no objects
        // yet to find.
        let Ok((tree, _)) = self.tree_now(doc) else {
            return;
        };
        let nodes: Vec<Node> = tree.iter().map(|(_, n)| n.clone()).collect();
        let review = self.review();
        review.looked_up.push(doc.to_owned());
        let mut changed = false;
        for c in review.comments.iter_mut().filter(|c| c.doc == doc) {
            if c.orphaned || review.bound.contains_key(&c.id) {
                continue;
            }
            match find_by_key(&nodes, &c.anchor.key) {
                Some(node) => {
                    let (handle, _) = tree
                        .iter()
                        .find(|(_, n)| n.path == node.path)
                        .expect("the tree's own node");
                    let n = handle[1..].parse().expect("handles are n and a number");
                    review.bound.insert(c.id.clone(), n);
                }
                // Changed while Studio was not looking: the last place
                // written down is all there is.
                None => {
                    c.orphaned = true;
                    changed = true;
                }
            }
        }
        if changed {
            self.save_derived();
        }
    }

    /// [`look_up`](Self::look_up) for a reader of comments: the document is
    /// brought level with its file first. One that cannot be (gone, or not
    /// a document) has no objects to find, and its comments stay where the
    /// sidecar last put them; a reply to one is still a reply.
    fn look_up_now(&mut self, doc: &str) {
        if self.sync(doc).is_ok() {
            self.look_up(doc);
        }
    }

    /// After `doc` changed: its comments take their objects' places now, or
    /// are orphaned where their objects were taken out.
    ///
    /// The design's edit is made whatever becomes of the sidecar: a
    /// comment not written down now is in the session still, and written
    /// with the next one.
    fn reanchor(&mut self, doc: &str) {
        let Some(review) = &mut self.review else {
            return;
        };
        if !review.comments.iter().any(|c| c.doc == doc) {
            return;
        }
        let document = self.docs.get(doc);
        // While the text does not parse, handles wait, and so do comments.
        if document.is_some_and(|d| d.nodes.is_none()) {
            return;
        }
        let mut changed = false;
        for c in review.comments.iter_mut().filter(|c| c.doc == doc) {
            let Some(handle) = review.bound.get(&c.id) else {
                continue;
            };
            let node = document.and_then(|d| {
                let path = d
                    .tracked
                    .iter()
                    .find(|t| t.handle == *handle)?
                    .path
                    .as_ref()?;
                d.nodes.as_ref()?.iter().find(|n| n.path == *path)
            });
            let (anchor, orphaned) = match node {
                Some(node) => (anchor(node), false),
                None => (c.anchor.clone(), true),
            };
            if anchor != c.anchor || orphaned != c.orphaned {
                c.anchor = anchor;
                c.orphaned = orphaned;
                changed = true;
            }
        }
        if changed {
            self.save_derived();
        }
    }

    /// Writes the comments down. The sidecar is not the design: nothing
    /// that shows the design is told.
    ///
    /// A sidecar somebody else changed since it was read is never written
    /// over: it is read again and merged instead, and the caller told.
    fn save_review(&mut self) -> Result<(), Error> {
        let now = self.read_review();
        let review = self.review();
        if let Some(why) = &review.unreadable {
            return Err(Error::ReviewUnreadable(why.clone()));
        }
        if now.seen != review.seen {
            let held = self.review.take().expect("just looked at");
            self.review = Some(held.merged(now));
            return Err(Error::ReviewChanged);
        }
        let text = review.to_text();
        self.store.write(review::FILE, &text)?;
        self.review().seen = Some(text);
        Ok(())
    }

    fn view(&self, comment: &Comment) -> CommentView {
        let node = match &self.review {
            Some(review) if !comment.orphaned => {
                review.bound.get(&comment.id).map(|h| format!("n{h}"))
            }
            _ => None,
        };
        CommentView {
            comment: comment.clone(),
            node,
        }
    }

    /// The comments on `doc`'s objects, or on every document's, in the
    /// order they were made.
    ///
    /// A sidecar that does not read is refused rather than answered as no
    /// comments: they are there, only not to be read.
    pub fn comments(&mut self, doc: Option<&str>) -> Result<Vec<CommentView>, Error> {
        self.refresh_review();
        if let Some(why) = &self.review().unreadable {
            return Err(Error::ReviewUnreadable(why.clone()));
        }
        let docs: Vec<String> = {
            let review = self.review();
            let mut docs: Vec<String> = review
                .comments
                .iter()
                .filter(|c| doc.is_none_or(|d| c.doc == d))
                .map(|c| c.doc.clone())
                .collect();
            docs.sort();
            docs.dedup();
            docs
        };
        for d in &docs {
            self.look_up_now(d);
        }
        let review = self.review.as_ref().expect("read above");
        Ok(review
            .comments
            .iter()
            .filter(|c| doc.is_none_or(|d| c.doc == d))
            .map(|c| self.view(c))
            .collect())
    }

    /// A new comment on the object `address` names, written at `at`
    /// (milliseconds since the Unix epoch). The caller reads the clock: a
    /// session has none of its own, as a browser gives none to `std`.
    pub fn comment_add(
        &mut self,
        doc: &str,
        address: &Address,
        text: &str,
        author: Option<&str>,
        at: u64,
    ) -> Result<CommentView, Error> {
        let text = note_text(text)?;
        self.refresh_review();
        self.sync(doc)?;
        self.look_up(doc);
        let (handle, node) = self.resolve_now(doc, address)?;
        self.check_leased(doc, [number(&handle)])?;
        let review = self.review();
        if let Some(why) = &review.unreadable {
            return Err(Error::ReviewUnreadable(why.clone()));
        }
        let comment = Comment {
            id: review.next_id(),
            doc: doc.to_owned(),
            anchor: anchor(&node),
            orphaned: false,
            resolved: false,
            thread: vec![Note {
                author: author.map(str::to_owned),
                text,
                at,
            }],
        };
        let n = handle[1..].parse().expect("handles are n and a number");
        review.bound.insert(comment.id.clone(), n);
        review.comments.push(comment.clone());
        // A comment the sidecar did not take is not made: kept, it would
        // be written with the next, and a caller trying again after the
        // refusal would have made it twice.
        if let Err(e) = self.save_review() {
            if let Some(review) = &mut self.review {
                review.comments.retain(|c| c.id != comment.id);
                review.bound.remove(&comment.id);
            }
            return Err(e);
        }
        Ok(self.view(&comment))
    }

    /// Answers the comment `id`, an orphaned one too, at `at` (as
    /// [`Session::comment_add`]).
    pub fn comment_reply(
        &mut self,
        id: &str,
        text: &str,
        author: Option<&str>,
        at: u64,
    ) -> Result<CommentView, Error> {
        let text = note_text(text)?;
        self.change_comment(id, |c| {
            c.thread.push(Note {
                author: author.map(str::to_owned),
                text,
                at,
            });
        })
    }

    /// Resolves the comment `id`, or reopens it.
    pub fn comment_resolve(&mut self, id: &str, resolved: bool) -> Result<CommentView, Error> {
        self.change_comment(id, |c| c.resolved = resolved)
    }

    fn change_comment(
        &mut self,
        id: &str,
        f: impl FnOnce(&mut Comment),
    ) -> Result<CommentView, Error> {
        self.refresh_review();
        if let Some(why) = &self.review().unreadable {
            return Err(Error::ReviewUnreadable(why.clone()));
        }
        let doc = self
            .review()
            .comments
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.doc.clone())
            .ok_or_else(|| Error::NoComment(id.to_owned()))?;
        self.look_up_now(&doc);
        if let Some(h) = self.review().bound.get(id).copied() {
            self.check_leased(&doc, [h])?;
        }
        let comment = self.review().get_mut(id).expect("found above");
        let before = comment.clone();
        f(comment);
        let comment = comment.clone();
        // As with a new comment: a change the sidecar did not take is not
        // made.
        if let Err(e) = self.save_review() {
            if let Some(c) = self.review.as_mut().and_then(|r| r.get_mut(id)) {
                *c = before;
            }
            return Err(e);
        }
        Ok(self.view(&comment))
    }

    pub fn history(&self) -> (Vec<EntrySummary>, Vec<EntrySummary>) {
        self.history.list()
    }

    /// What `author`'s undo and redo would take, in [`Session::history`]'s
    /// order.
    pub fn history_by(&self, author: &str) -> (Vec<EntrySummary>, Vec<EntrySummary>) {
        self.history.list_by(author)
    }

    /// Leases the objects `handles` of `path` name, and all they hold, to
    /// the caller in place of what it held: another writer's edit of them
    /// is refused until the caller stops calling for [`LEASE_TTL_MS`], or
    /// [`LEASE_LIFE_MS`] went by. An object the caller already held keeps
    /// the time it was first held.
    ///
    /// # Errors
    /// An object another writer holds, or holds inside of, or a caller
    /// whose leases ran their life and who has not released since.
    pub fn hold(&mut self, path: &str, handles: &[String]) -> Result<(), Error> {
        if self.spent.contains(&self.author) {
            return Err(Error::LeaseSpent {
                holder: self.author.clone(),
            });
        }
        let path = &self.canonical(path)?;
        self.doc(path)?;
        let doc = &self.docs[path];
        let wanted: Vec<u64> = handles.iter().map(|h| number(h)).collect();
        for h in &wanted {
            if let Some(lease) = self
                .others_leases(path)
                .find(|l| within(doc, *h, l.handle) || within(doc, l.handle, *h))
            {
                return Err(self.leased(lease));
            }
        }
        let now = self.now_ms;
        let author = self.author.clone();
        let mut kept: Vec<Lease> = Vec::new();
        self.leases.retain(|l| {
            if l.holder != author {
                return true;
            }
            if l.doc == *path && wanted.contains(&l.handle) {
                kept.push(l.clone());
            }
            false
        });
        for h in wanted {
            let mut lease = kept
                .iter()
                .find(|l| l.handle == h)
                .cloned()
                .unwrap_or(Lease {
                    doc: path.clone(),
                    handle: h,
                    holder: author.clone(),
                    since_ms: now,
                    until_ms: now,
                });
            lease.renew(now);
            self.leases.push(lease);
        }
        Ok(())
    }

    /// Lets go of everything the caller holds; one whose leases ran their
    /// life may hold again.
    pub fn release(&mut self) {
        let author = self.author.clone();
        self.leases.retain(|l| l.holder != author);
        self.spent.retain(|h| *h != author);
    }

    /// The leases in force, every writer's.
    pub fn leases(&self) -> Vec<LeaseView> {
        self.leases
            .iter()
            .filter(|l| l.until_ms > self.now_ms)
            .map(|l| LeaseView {
                doc: l.doc.clone(),
                handle: format!("n{}", l.handle),
                holder: l.holder.clone(),
                until_ms: l.until_ms,
            })
            .collect()
    }

    /// The handles of `path` the caller holds.
    pub fn held(&self, path: &str) -> Vec<String> {
        self.leases
            .iter()
            .filter(|l| l.doc == path && l.holder == self.author && l.until_ms > self.now_ms)
            .map(|l| format!("n{}", l.handle))
            .collect()
    }

    /// The bytes each document's log holds and its number of entries, for
    /// measuring what rebasing costs.
    pub fn log_size(&self) -> (usize, usize) {
        self.logs
            .values()
            .fold((0, 0), |(b, n), l| (b + l.bytes(), n + l.len()))
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

    /// Adds each of `addresses` to the selection, or takes it out if it is
    /// in: what a shift-click does. A selection is of one document, as its
    /// box and its one undo step are: another document's is left behind.
    pub fn toggle_selected(
        &mut self,
        path: &str,
        addresses: &[Address],
    ) -> Result<Vec<String>, Error> {
        let path = &self.canonical(path)?;
        let mut handles: Vec<String> = self
            .selection
            .iter()
            .filter(|(doc, _)| doc == path)
            .map(|(_, handle)| handle.clone())
            .collect();
        for address in addresses {
            let handle = self.resolve(path, address)?.0;
            match handles.iter().position(|h| *h == handle) {
                Some(at) => {
                    handles.remove(at);
                }
                None => handles.push(handle),
            }
        }
        self.selection = handles
            .iter()
            .map(|h| (path.to_owned(), h.clone()))
            .collect();
        Ok(handles)
    }

    /// The selection as (document, handle), dropping objects that are gone.
    /// While a document's text does not parse its handles wait, and what they
    /// select waits with them, out of the answer until the text parses again.
    pub fn selection(&mut self) -> Vec<(String, String)> {
        let selection = std::mem::take(&mut self.selection);
        let mut now = Vec::new();
        for (doc, handle) in selection {
            if self.resolve(&doc, &Address::Node(handle.clone())).is_ok() {
                now.push((doc.clone(), handle.clone()));
            } else if !self.docs.get(&doc).is_some_and(|d| d.nodes.is_none()) {
                continue;
            }
            self.selection.push((doc, handle));
        }
        now
    }
}

/// The refusal of an edit `entry` of the log stands in the way of.
fn conflict(doc: &str, entry: &Logged, now: Option<String>) -> Error {
    let (handle, name) = match entry.touched.as_deref() {
        Some([(h, name)]) => (Some(format!("n{h}")), name.clone()),
        _ => (None, None),
    };
    Error::Conflict(Box::new(Conflict {
        doc: doc.to_owned(),
        handle,
        name,
        author: entry.author.clone(),
        label: entry.label.clone(),
        generation: entry.generation,
        now,
    }))
}

/// The number of a handle the session gave out (`n12`).
fn number(handle: &str) -> u64 {
    handle[1..].parse().expect("handles are n and a number")
}

/// The objects written in the value of `handle`'s binding `name`, a group's
/// (`font`) naming those of its members (`font.family`) too.
fn in_binding<'a>(doc: &'a Document, handle: u64, name: &'a str) -> impl Iterator<Item = u64> + 'a {
    let at = doc
        .tracked
        .iter()
        .find(|t| t.handle == handle)
        .and_then(|t| t.path.as_ref());
    doc.tracked.iter().filter_map(move |t| {
        let (at, path) = (at?, t.path.as_ref()?);
        let below = path.0.strip_prefix(at.0.as_slice())?;
        let Some(qml_edit::Step::Value(step)) = below.first() else {
            return None;
        };
        let inside = step == name
            || step
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with('.'));
        inside.then_some(t.handle)
    })
}

/// Whether the object `inner` is `outer` or one it holds, by where their
/// handles point now; a handle whose object is gone is only itself.
fn within(doc: &Document, inner: u64, outer: u64) -> bool {
    if inner == outer {
        return true;
    }
    let at = |h: u64| {
        doc.tracked
            .iter()
            .find(|t| t.handle == h)
            .and_then(|t| t.path.as_ref())
    };
    match (at(inner), at(outer)) {
        (Some(i), Some(o)) => i.0.len() > o.0.len() && i.0.starts_with(&o.0),
        _ => false,
    }
}

/// A hash of a text, to know a history entry's side of it again.
fn side(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Where a comment on `node` is.
fn anchor(node: &Node) -> Anchor {
    Anchor {
        key: node.key.clone(),
        path: node.path.to_string(),
        line: node.line,
        ty: node.ty.clone(),
        id: node.id.clone(),
    }
}

fn note_text(text: &str) -> Result<String, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Invalid("a comment needs text".to_owned()));
    }
    Ok(text.to_owned())
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
        if let Some(at) = found {
            taken[at] = true;
            placed[i] = Some(Some(nodes[at].path.clone()));
        }
    }
    // An object no key names, its `id` renamed by a text written over it, is
    // where its start was.
    for (i, t) in doc.tracked.iter().enumerate() {
        if placed[i].is_some() {
            continue;
        }
        let Some(head) = t.head else { continue };
        if let Some(at) = nodes
            .iter()
            .position(|n| n.start == head && n.ty == t.ty)
            .filter(|&at| !taken[at])
        {
            taken[at] = true;
            placed[i] = Some(Some(nodes[at].path.clone()));
        }
    }
    for p in &mut placed {
        p.get_or_insert(None);
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
            t.head = None;
        }
        None => {
            t.span = None;
            t.head = None;
        }
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

/// The name of the member `text` declares (a function, a signal, a property
/// or a signal handler); `None` for an object, which starts with its type.
fn member_name(text: &str) -> Option<&str> {
    let ident = |s: &str| {
        let end = s
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(s.len());
        (end > 0).then_some(end)
    };
    let text = text.trim();
    let mut words = text.split_whitespace();
    let mut first = words.next()?;
    while matches!(first, "readonly" | "required" | "default") {
        first = words.next()?;
    }
    let after = |word: &str, skip_type: bool| {
        let at = text.find(word)? + word.len();
        let mut rest = text[at..].trim_start();
        if skip_type {
            rest = rest[rest.find(char::is_whitespace)?..].trim_start();
        }
        Some(&rest[..ident(rest)?])
    };
    match first {
        "function" => after("function", false),
        "signal" => after("signal", false),
        "property" => after("property", true),
        _ => {
            let end = ident(text)?;
            let handler = text.starts_with("on")
                && text[2..].starts_with(|c: char| c.is_ascii_uppercase())
                && text[end..].trim_start().starts_with(':');
            handler.then(|| &text[..end])
        }
    }
}

fn has_member(object: &outline::Node, name: &str) -> bool {
    object.functions.iter().any(|f| f == name)
        || object.properties.iter().any(|p| p == name)
        || object.signals.iter().any(|s| s.name == name)
        || object.bindings.iter().any(|b| b.name == name)
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
