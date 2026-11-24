//! What a document opened alone needs before it shows anything.
//!
//! A delegate or a component that wants `required property`s set fails to
//! load as an entry: nothing sets them. A store (a block that holds a model
//! and functions) loads, and shows an empty window. Either one is previewed
//! through a harness: a file beside it that makes one instance, with sample
//! values for what it requires, and, for a store, a sheet of what it offers
//! read live from that instance. The harness is a dot-file, which Studio's
//! lists, its watcher and an export all pass over, while `ljos run` takes
//! it as an entry like any other; the component's directory is its own, so
//! it sees the same types and imports.

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use studio_core::{
    Session, Step, Store,
    outline::{self, Member, MemberKind, Node},
};

/// Sample values given once for a document, by its path and then by
/// property name, each a JSON value.
pub(crate) const SAMPLES: &str = ".studio/samples.json";

/// How each harness starts, which tells Studio's file from a person's.
const MARK: &str = "// Studio's preview of ";

/// What a harness imports QtQuick as. The document's type is named bare,
/// which only its own directory then answers: imported bare, QtQuick's
/// `Row` would be made for a `Row.qml`.
const QUICK: &str = "StudioQuick";

/// Types that show nothing: an object holding only these is a store.
const SHOW_NOTHING: &[&str] = &[
    "QtObject",
    "ListModel",
    "JsonListModel",
    "Timer",
    "Connections",
    "Instantiator",
    "Binding",
    "Settings",
    "WorkerScript",
    "Component",
    "Contribution",
    "ItemSelectionModel",
    "FontLoader",
    "Shortcut",
];

/// What a harness stands in for a document with: its path and its text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Harness {
    pub path: String,
    pub text: String,
}

impl Harness {
    /// This harness as written into the project at `dir`. Made before the
    /// file is written: what holds it is seen by another Studio ending its
    /// own preview of the component before the file is there to remove.
    /// None when the project has a directory and the file cannot be held
    /// there: written unheld, another Studio's ending preview would take it
    /// from under this one.
    pub fn standing(&self, dir: Option<&Path>) -> Option<Standing> {
        let file = dir.map(|dir| {
            let root = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
            (root, dir.join(&self.path))
        });
        let held = match &file {
            Some((root, file)) => Some(hold(root, file)?),
            None => None,
        };
        Some(Standing {
            path: self.path.clone(),
            file,
            held,
        })
    }
}

/// A written harness. Its file goes when this does, so that each way a
/// preview ends (another one, a crash, another project, Studio closing)
/// takes it along: not all of those have the project's store in hand.
#[derive(Debug)]
pub(crate) struct Standing {
    pub path: String,
    /// The project's directory as it was when the file was written, and the
    /// file. None in a project without a directory, which only its store
    /// reaches.
    file: Option<(PathBuf, PathBuf)>,
    /// This preview's hold on the file, which another Studio previewing the
    /// same component from the project sees and so leaves the file.
    held: Option<Held>,
}

/// A lock file beside a harness, locked while a preview runs from it. A
/// lock, not a pid, says it lives: a killed Studio's is taken by the next
/// that looks, as Studio's state directory is (`studio/src/state.rs`).
#[derive(Debug)]
struct Held {
    lock: File,
    path: PathBuf,
}

/// The project's preview gate, beside its samples in Studio's sidecar.
const GATE: &str = ".studio/preview.lock";

/// Takes the gate of the project at `root`: held while a preview makes its
/// hold, and while one that ends counts the holds and removes the file, so
/// no hold is made between that count and that removal. Waits for another
/// Studio's turn, which is a few file operations long. None when there is
/// none to take: the sidecar cannot be made, or leads out of the project.
fn gate(root: &Path) -> Option<File> {
    let dir = root.join(".studio");
    std::fs::create_dir_all(&dir).ok()?;
    if !dir.canonicalize().ok()?.starts_with(root) {
        return None;
    }
    let path = root.join(GATE);
    // Made only where nothing is: an exclusive create follows no link.
    let _ = File::options().write(true).create_new(true).open(&path);
    let gate = plain(&path)?;
    gate.lock().ok()?;
    Some(gate)
}

/// Opens the plain file at `path` to read, never what a link there leads
/// to: whoever else writes to the project could plant one leading out of
/// it. Looked at before the open, so no link is followed into a pipe that
/// would never open, and after, so one swapped in meanwhile is caught by
/// not being what was opened.
fn plain(path: &Path) -> Option<File> {
    if !path.symlink_metadata().ok()?.file_type().is_file() {
        return None;
    }
    let file = File::open(path).ok()?;
    let here = path.symlink_metadata().ok()?;
    (here.file_type().is_file() && same(&here, &file.metadata().ok()?)).then_some(file)
}

/// Whether two looks at a file saw the same one.
#[cfg(unix)]
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// Whether two looks at a file saw the same one. Without a file's identity
/// at hand, the look before the open stands alone.
#[cfg(not(unix))]
fn same(_: &std::fs::Metadata, _: &std::fs::Metadata) -> bool {
    true
}

/// What a hold beside `file` is named: `<file>.<run>.lock`.
fn hold_path(file: &Path, run: &str) -> Option<PathBuf> {
    let mut name = file.file_name()?.to_os_string();
    name.push(format!(".{run}.lock"));
    Some(file.with_file_name(name))
}

/// Holds `file` for this preview, under the gate of the project at `root`.
/// Nothing counts the holds meanwhile, so it is made in place: one a killed
/// Studio made and did not lock is a file nobody holds, which the next
/// count removes as a dead hold's.
fn hold(root: &Path, file: &Path) -> Option<Held> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let _gate = gate(root)?;
    // Only in a directory still inside the project: one swapped for a link
    // would lead out of it.
    if !file.parent()?.canonicalize().ok()?.starts_with(root) {
        return None;
    }
    // Unique among the holds of live runs: a pid is not reused while its
    // run lives, and the count tells this run's apart.
    let run = format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let path = hold_path(file, &run)?;
    // Exclusive: never truncates a file, nor follows a link, already there.
    let lock = File::options()
        .write(true)
        .create_new(true)
        .open(&path)
        .ok()?;
    if lock.try_lock().is_err() {
        // std opens files shareable for deletion, so it goes while open.
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(Held { lock, path })
}

/// Whether a preview other than this one runs from `file`. A killed
/// Studio's hold found on the way is removed.
fn held_by_another(file: &Path) -> bool {
    let (Some(dir), Some(name)) = (file.parent(), file.file_name()) else {
        return false;
    };
    let prefix = format!("{}.", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut held = false;
    for entry in entries.flatten() {
        let found = entry.file_name();
        let found = found.to_string_lossy();
        // Not followed: a link here is nobody's hold.
        if !found.starts_with(&prefix)
            || !found.ends_with(".lock")
            || !entry.file_type().is_ok_and(|t| t.is_file())
        {
            continue;
        }
        let Some(lock) = plain(&entry.path()) else {
            continue;
        };
        match lock.try_lock() {
            // Taken: its preview has ended, or its Studio was killed before
            // it locked it.
            Ok(()) => {
                let _ = std::fs::remove_file(entry.path());
            }
            // Refused: it lives, or it cannot be judged, and a file is
            // never removed on a guess.
            Err(_) => held = true,
        }
    }
    held
}

/// Leave to remove a harness: the project's gate, held until it has gone.
struct Leave {
    _gate: Option<File>,
}

impl Standing {
    /// Lets go of this preview's hold. Some when the file is this
    /// preview's to remove: no other preview holds it, or it is outside the
    /// project, where another's holds are not this Studio's to judge.
    fn release(&mut self) -> Option<Leave> {
        let gate = match &self.file {
            Some((root, _)) if self.inside() => gate(root),
            _ => None,
        };
        if let Some(held) = self.held.take() {
            // Let go before it goes, so whoever looks next may take it.
            let _ = held.lock.unlock();
            if self.inside() {
                let _ = std::fs::remove_file(&held.path);
            }
        }
        let another = self.inside()
            && self
                .file
                .as_ref()
                .is_some_and(|(_, file)| held_by_another(file));
        (!another).then_some(Leave { _gate: gate })
    }

    /// Whether the file's directory is still inside the project. The
    /// store's confinement is not at hand here: a directory swapped for a
    /// link since would lead out of it.
    fn inside(&self) -> bool {
        self.file.as_ref().is_some_and(|(root, file)| {
            file.parent()
                .and_then(|dir| dir.canonicalize().ok())
                .is_some_and(|d| d.starts_with(root))
        })
    }

    /// Takes the file out through the project's store, which alone reaches
    /// it in a project without a directory. A person's file there stays,
    /// and so does one another Studio's preview runs from.
    pub fn leave(&mut self, store: &dyn Store) {
        if let Some(_leave) = self.release()
            && store.read(&self.path).is_ok_and(|text| is_studios(&text))
        {
            // Already gone is as good.
            let _ = store.remove(&self.path);
        }
        // Left: dropping it then has nothing more to do.
        self.file = None;
    }
}

impl Drop for Standing {
    fn drop(&mut self) {
        let Some(_leave) = self.release() else {
            return;
        };
        if let Some((_, file)) = self.file.as_ref().filter(|_| self.inside()) {
            // A person may have written their own file there since, or
            // put a link there, which Studio never writes.
            if plain(file)
                .and_then(|opened| std::io::read_to_string(opened).ok())
                .is_some_and(|text| is_studios(&text))
            {
                // Already gone is as good.
                let _ = std::fs::remove_file(file);
            }
        }
    }
}

/// Whether `text` is a harness Studio wrote, and so its to replace or remove.
pub(crate) fn is_studios(text: &str) -> bool {
    text.starts_with(MARK)
}

/// The project's documents without what Studio itself stands in for one. A
/// store lists a hidden `.qml` file as any other, and a person's is theirs.
pub(crate) fn documents(session: &Session) -> Vec<String> {
    let store = session.store();
    let mut documents = session.documents();
    documents.retain(|path| !stands_in(store, path));
    documents
}

/// Whether the file at `path` is a harness Studio wrote.
fn stands_in(store: &dyn Store, path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.starts_with('.')
        && file.ends_with(".preview.qml")
        && store.read(path).is_ok_and(|text| is_studios(&text))
}

/// The line, one-based, a harness's text declares its one instance of the
/// document on, as a runtime names that object.
pub(crate) fn instance_line(text: &str) -> Option<usize> {
    // The instance's `id: subject` is on the line after its own: its index
    // counted from 0 is the instance's line counted from 1.
    text.lines().position(|line| line.trim() == "id: subject")
}

fn short(ty: &str) -> &str {
    ty.rsplit('.').next().unwrap_or(ty)
}

/// The harness `doc` is previewed through, or `None` when it shows itself.
pub(crate) fn harness(store: &dyn Store, doc: &str) -> Option<Harness> {
    let (dir, file) = match doc.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, doc),
    };
    let name = file.strip_suffix(".qml")?;
    if !name.starts_with(|c: char| c.is_ascii_uppercase())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let text = store.read(doc).ok()?;
    let nodes = outline::outline(&text)?;
    let root = nodes.first()?;
    let root_ty = short(&root.ty);
    // What the chain of the project's own bases ends in says what it is: a
    // `Base {}` of a `QtObject` shows nothing, and has no size to be given.
    let base_ty = toolkit_base(store, doc);
    let is = |names: &[&str]| {
        names.contains(&root_ty) || base_ty.as_deref().is_some_and(|ty| names.contains(&ty))
    };
    if is(&["Window", "ApplicationWindow"]) {
        return None;
    }
    let mut members = outline::members(&text, root.start);
    members.extend(inherited(store, doc));
    let samples = samples(store, doc);
    let sets = sets(&members, &samples);
    let block = is_block(store, dir, file);
    let store_like = is(SHOW_NOTHING) || shows_nothing(&nodes, root, &members, block);
    let requires = members
        .iter()
        .any(|m| m.kind == MemberKind::Property && m.required);
    let text = if store_like {
        sheet(name, &members, &sets)
    } else if requires || block {
        // A block is a part, not an app: shown as itself, its root would be
        // stretched to the whole frame, a card as wide as a screen.
        instance(name, root, &sets)
    } else {
        return None;
    };
    let path = match dir {
        Some(dir) => format!("{dir}/.{name}.preview.qml"),
        None => format!(".{name}.preview.qml"),
    };
    // A file of that name that Studio did not write is the person's.
    if store.exists(&path) && !store.read(&path).is_ok_and(|old| is_studios(&old)) {
        return None;
    }
    Some(Harness { path, text })
}

/// The type the root of the last of `doc`'s bases in the project is, when
/// it has any: one Studio does not find in the project's files.
fn toolkit_base(store: &dyn Store, doc: &str) -> Option<String> {
    let lineage = crate::signals::lineage(store, doc);
    let last = lineage.last().filter(|last| *last != doc)?;
    let text = store.read(last).ok()?;
    let root = outline::outline(&text)?.into_iter().next()?;
    Some(short(&root.ty).to_owned())
}

/// Whether the `qmldir` beside the file declares it a block.
fn is_block(store: &dyn Store, dir: Option<&str>, file: &str) -> bool {
    let qmldir = dir.map_or_else(|| "qmldir".to_owned(), |dir| format!("{dir}/qmldir"));
    store.read(&qmldir).is_ok_and(|text| {
        text.lines().any(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            matches!(words.as_slice(), ["#", "block", _, _, f, ..] if *f == file)
        })
    })
}

/// A root that shows nothing: a type that never does, or a block that is a
/// plain `Item` with no size, whose children show nothing either, which
/// offers something. A document that is no block is left to show itself:
/// an app's root may well start out so.
fn shows_nothing(nodes: &[Node], root: &Node, members: &[Member], block: bool) -> bool {
    let ty = short(&root.ty);
    if SHOW_NOTHING.contains(&ty) {
        return true;
    }
    let sized = root.bindings.iter().any(|b| {
        matches!(
            b.name.as_str(),
            "width" | "height" | "implicitWidth" | "implicitHeight"
        ) || b.name.starts_with("anchors")
    });
    let children_show_nothing = nodes
        .iter()
        .filter(|n| matches!(n.path.0.as_slice(), [Step::Child(_)]))
        .all(|n| SHOW_NOTHING.contains(&short(&n.ty)));
    block
        && ty == "Item"
        && !sized
        && children_show_nothing
        && members.iter().any(|m| m.kind != MemberKind::Handler)
}

/// What a document `doc` made around the block in `file` sets on it, so
/// that it can be made: each property it requires, as a harness sets it.
pub(crate) fn required_sets(store: &dyn Store, file: &str, doc: &str) -> Vec<(String, String)> {
    let Ok(text) = store.read(file) else {
        return Vec::new();
    };
    let Some(root) = outline::outline(&text).and_then(|nodes| nodes.into_iter().next()) else {
        return Vec::new();
    };
    let mut required: Vec<Member> = outline::members(&text, root.start)
        .into_iter()
        .filter(|m| m.required)
        .collect();
    required.extend(inherited(store, file));
    let mut given = samples(store, file);
    // A url is resolved against the file that binds it, and the samples
    // were given for a harness beside the block: from elsewhere the same
    // picture is reached by another path. A string stays as given.
    for member in required.iter().filter(|m| m.ty.as_deref() == Some("url")) {
        if let Some(Value::String(url)) = given.get_mut(&member.name)
            && let Some(moved) = rebased(url, directory(file), directory(doc))
        {
            *url = moved;
        }
    }
    sets(&required, &given)
}

fn directory(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// A relative `url`, said in the directory `from`, as said in `to` (both
/// relative to the project). None for one that is absolute, has a scheme,
/// or leaves the project: it reads the same anywhere, or is kept as given.
fn rebased(url: &str, from: &str, to: &str) -> Option<String> {
    let scheme = url
        .split_once(':')
        .is_some_and(|(head, _)| !head.is_empty() && !head.contains('/'));
    if url.is_empty() || url.starts_with('/') || scheme {
        return None;
    }
    let mut target: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    for part in url.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                target.pop()?;
            }
            part => target.push(part),
        }
    }
    let (_, dirs) = target.split_last()?;
    let here: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let shared = here.iter().zip(dirs).take_while(|(a, b)| a == b).count();
    let mut parts = vec![".."; here.len() - shared];
    parts.extend(&target[shared..]);
    let mut moved = parts.join("/");
    // A directory keeps its slash: a binding appends a name to it.
    if url.ends_with('/') {
        moved.push('/');
    }
    Some(moved)
}

/// What the bases of the component in `file` require that neither it nor
/// a base between them binds or declares again: QML makes nothing with a
/// required property unset, wherever it was declared.
fn inherited(store: &dyn Store, file: &str) -> Vec<Member> {
    let mut required = Vec::new();
    let mut settled = HashSet::new();
    for (i, link) in crate::signals::lineage(store, file).iter().enumerate() {
        let Ok(text) = store.read(link) else {
            continue;
        };
        let Some(root) = outline::outline(&text).and_then(|nodes| nodes.into_iter().next()) else {
            continue;
        };
        let members = outline::members(&text, root.start);
        if i > 0 {
            required.extend(
                members
                    .iter()
                    .filter(|m| m.required && !settled.contains(&m.name))
                    .cloned(),
            );
        }
        settled.extend(members.into_iter().map(|m| m.name));
        settled.extend(root.bindings.into_iter().map(|b| b.name));
    }
    required
}

fn samples(store: &dyn Store, doc: &str) -> serde_json::Map<String, Value> {
    store
        .read(SAMPLES)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|all| all.get(doc).and_then(Value::as_object).cloned())
        .unwrap_or_default()
}

/// What the harness sets on the instance: a sample given for any declared
/// property, else a value of its type for a required one.
fn sets(members: &[Member], samples: &serde_json::Map<String, Value>) -> Vec<(String, String)> {
    members
        .iter()
        .filter(|m| m.kind == MemberKind::Property && !m.readonly)
        .filter(|m| m.ty.as_deref() != Some("alias") || samples.contains_key(&m.name))
        .filter_map(|m| {
            let value = match samples.get(&m.name) {
                Some(given) => written(given),
                None if m.required => typed(m.ty.as_deref().unwrap_or("var"), &m.name),
                None => return None,
            };
            Some((m.name.clone(), value))
        })
        .collect()
}

fn written(value: &Value) -> String {
    match value {
        // An object literal in a binding is read as a block without these.
        Value::Object(_) => format!("({value})"),
        _ => value.to_string(),
    }
}

/// `dueDate` as a person would title it: "Due date".
fn titled(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_ascii_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else if c == '_' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// A value of `ty` that shows: a property's name for a text, small numbers.
fn typed(ty: &str, name: &str) -> String {
    match ty {
        "string" | "var" | "variant" => Value::String(titled(name)).to_string(),
        "bool" => "false".to_owned(),
        "int" if name == "index" => "0".to_owned(),
        "int" => "3".to_owned(),
        "real" | "double" | "number" => "0.5".to_owned(),
        "color" => "\"#6750a4\"".to_owned(),
        "url" => "\"\"".to_owned(),
        "date" => "new Date()".to_owned(),
        _ if ty.starts_with("list<") => "[]".to_owned(),
        _ => "null".to_owned(),
    }
}

fn instance(name: &str, root: &Node, sets: &[(String, String)]) -> String {
    // A delegate sizes itself by its view, which a harness has not got: it
    // is given a card's size, as one with no size at all is.
    let own = |side: &str, implicit: &str| {
        root.bindings.iter().any(|b| {
            b.name == implicit
                || (b.name == side
                    && ![
                        "ListView.",
                        "GridView.",
                        "PathView.",
                        "TableView.",
                        "parent",
                    ]
                    .iter()
                    .any(|view| b.value.contains(view)))
        })
    };
    let mut body = String::new();
    if !own("width", "implicitWidth") {
        body.push_str("        width: 320\n");
    }
    if !own("height", "implicitHeight") {
        body.push_str("        height: 72\n");
    }
    for (property, value) in sets {
        body.push_str(&format!("        {property}: {value}\n"));
    }
    // The preview sizes the root to its frame, a whole screen wider than
    // one card: the card stands in the middle of it, as on a board.
    format!(
        "{MARK}{name}.qml opened alone, with sample values.\n\
         import QtQuick as {QUICK}\n\n\
         {QUICK}.Item {{\n    width: subject.width\n    height: subject.height\n\n    \
         {name} {{\n        id: subject\n        anchors.centerIn: parent\n{body}    }}\n}}\n"
    )
}

fn quoted(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

fn sheet(name: &str, members: &[Member], sets: &[(String, String)]) -> String {
    let set: String = sets
        .iter()
        .map(|(property, value)| format!("        {property}: {value}\n"))
        .collect();
    let mut lines = Vec::new();
    let heading = |lines: &mut Vec<String>, kind: MemberKind, title: &str| {
        let of_kind: Vec<&Member> = members.iter().filter(|m| m.kind == kind).collect();
        if of_kind.is_empty() {
            return;
        }
        lines.push(format!(
            "        {QUICK}.Text {{ text: {}; font.bold: true; topPadding: 8; color: \"#1c1b1f\"; {QUICK}.Accessible.role: {QUICK}.Accessible.StaticText }}",
            quoted(title)
        ));
        for m in of_kind {
            let line = match kind {
                MemberKind::Property => format!(
                    "{} + root.shown(subject.{})",
                    quoted(&format!(
                        "{}{}: ",
                        m.name,
                        m.ty.as_deref()
                            .map(|ty| format!(" ({ty})"))
                            .unwrap_or_default()
                    )),
                    m.name
                ),
                MemberKind::Signal | MemberKind::Function => quoted(&format!(
                    "{}({})",
                    m.name,
                    m.params.as_deref().unwrap_or_default()
                )),
                MemberKind::Handler => return,
            };
            lines.push(format!(
                "        {QUICK}.Text {{ text: {line}; color: \"#1c1b1f\"; width: parent.width; wrapMode: {QUICK}.Text.Wrap; {QUICK}.Accessible.role: {QUICK}.Accessible.StaticText }}"
            ));
        }
    };
    heading(&mut lines, MemberKind::Property, "Properties");
    heading(&mut lines, MemberKind::Function, "Functions");
    heading(&mut lines, MemberKind::Signal, "Signals");
    let lines = lines.join("\n");
    let title = quoted(name);
    format!(
        "{MARK}{name}.qml opened alone: it shows nothing itself,\n\
         // so this is what it offers, read from a live instance.\n\
         import QtQuick as {QUICK}\n\n\
         {QUICK}.Rectangle {{\n    id: root\n    width: 420\n    height: sheet.implicitHeight + 32\n    \
         color: \"#f6f6f8\"\n\n    \
         function shown(value) {{\n        \
         if (value === null || value === undefined)\n            return String(value)\n        \
         if (value.count !== undefined)\n            return value.count + \" rows\"\n        \
         return String(value)\n    }}\n\n    \
         {name} {{\n        id: subject\n{set}    }}\n\n    \
         {QUICK}.Column {{\n        id: sheet\n        x: 16\n        y: 16\n        \
         width: parent.width - 32\n        spacing: 4\n\n        \
         {QUICK}.Text {{ text: {title}; font.pixelSize: 20; font.bold: true; color: \"#1c1b1f\"; {QUICK}.Accessible.role: {QUICK}.Accessible.StaticText }}\n        \
         {QUICK}.Text {{ text: \"Shows nothing itself. What it offers:\"; color: \"#5f5d66\"; {QUICK}.Accessible.role: {QUICK}.Accessible.StaticText }}\n\
         {lines}\n    }}\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use studio_core::MemStore;

    use super::*;

    fn store(files: &[(&str, &str)]) -> MemStore {
        let store = MemStore::default();
        for (path, text) in files {
            store.write(path, text).unwrap();
        }
        store
    }

    #[test]
    fn a_component_that_requires_properties_is_given_samples() {
        let store = store(&[(
            "Blocks/Card.qml",
            "import QtQuick\nRectangle {\n    required property string title\n    \
             required property bool done\n    property int count: 1\n}\n",
        )]);
        let h = harness(&store, "Blocks/Card.qml").unwrap();
        assert_eq!(h.path, "Blocks/.Card.preview.qml");
        assert!(h.text.contains("        width: 320\n        height: 72\n"));
        assert!(h.text.contains("title: \"Title\"\n"));
        assert!(h.text.contains("done: false\n"));
        assert!(!h.text.contains("count:"));
        assert!(outline::outline(&h.text).is_some());
        let line = instance_line(&h.text).unwrap();
        assert_eq!(h.text.lines().nth(line - 1).map(str::trim), Some("Card {"));
    }

    #[test]
    fn samples_given_once_win() {
        let store = store(&[
            (
                "Card.qml",
                "import QtQuick\nItem {\n    implicitWidth: 10\n    \
                 required property string title\n    property int count\n}\n",
            ),
            (
                SAMPLES,
                r#"{"Card.qml": {"title": "Read a book", "count": 4}}"#,
            ),
        ]);
        let h = harness(&store, "Card.qml").unwrap();
        assert!(h.text.contains("title: \"Read a book\"\n"));
        assert!(h.text.contains("count: 4\n"));
        assert!(!h.text.contains("width: 320"));
        assert!(h.text.contains("height: 72"));
    }

    #[test]
    fn a_relative_url_sample_is_read_from_the_blocks_directory() {
        let store = store(&[
            (
                "Blocks/Card.qml",
                "import QtQuick\nItem {\n    required property url icon\n    \
                 required property url back\n    required property url remote\n    \
                 required property string label\n}\n",
            ),
            (
                SAMPLES,
                r#"{"Blocks/Card.qml": {"icon": "icons/run.svg", "back": "../art/a.svg",
                    "remote": "https://example.com/a.svg", "label": "icons/run.svg"}}"#,
            ),
        ]);
        let set = |doc: &str, name: &str| {
            required_sets(&store, "Blocks/Card.qml", doc)
                .into_iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v)
        };
        let said = |v: &str| Some(format!("\"{v}\""));
        assert_eq!(set("CardShell.qml", "icon"), said("Blocks/icons/run.svg"));
        assert_eq!(set("CardShell.qml", "back"), said("art/a.svg"));
        assert_eq!(
            set("Other/CardShell.qml", "icon"),
            said("../Blocks/icons/run.svg")
        );
        assert_eq!(set("Blocks/CardShell.qml", "icon"), said("icons/run.svg"));
        // Not relative, or not a url: read the same anywhere.
        assert_eq!(
            set("CardShell.qml", "remote"),
            said("https://example.com/a.svg")
        );
        assert_eq!(set("CardShell.qml", "label"), said("icons/run.svg"));
    }

    #[test]
    fn a_directory_url_sample_keeps_its_trailing_slash() {
        let store = store(&[
            (
                "Blocks/Card.qml",
                "import QtQuick\nItem {\n    required property url assetBase\n}\n",
            ),
            (SAMPLES, r#"{"Blocks/Card.qml": {"assetBase": "icons/"}}"#),
        ]);
        let set = |doc: &str| {
            required_sets(&store, "Blocks/Card.qml", doc)
                .into_iter()
                .find(|(n, _)| n == "assetBase")
                .map(|(_, v)| v)
        };
        // A binding appends a name to it: without the slash it names
        // another file.
        assert_eq!(set("Blocks/CardShell.qml").as_deref(), Some("\"icons/\""));
        assert_eq!(set("CardShell.qml").as_deref(), Some("\"Blocks/icons/\""));
        assert_eq!(
            set("Other/CardShell.qml").as_deref(),
            Some("\"../Blocks/icons/\"")
        );
    }

    #[test]
    fn a_component_of_a_base_that_shows_nothing_is_shown_as_what_it_offers() {
        let store = store(&[
            (
                "Base.qml",
                "import QtQml\nQtObject {\n    required property string title\n}\n",
            ),
            ("Derived.qml", "import QtQml\nBase {}\n"),
            (
                "Win.qml",
                "import QtQuick\nWindow {\n    required property int n\n}\n",
            ),
            ("Shown.qml", "import QtQuick\nWin {}\n"),
        ]);
        let h = harness(&store, "Derived.qml").expect("it requires a title");
        assert!(h.text.contains("What it offers"), "{}", h.text);
        // A QtObject has no size to give nor anchors to centre it by.
        assert!(!h.text.contains("anchors.centerIn"), "{}", h.text);
        assert!(h.text.contains("title: \"Title\""), "{}", h.text);
        // A window is its own frame, whatever it is named.
        assert_eq!(harness(&store, "Shown.qml"), None);
    }

    #[test]
    fn a_block_is_given_what_its_base_requires_and_it_does_not_set() {
        let store = store(&[
            (
                "Blocks/Base.qml",
                "import QtQuick\nItem {\n    required property string title\n    \
                 required property int count\n    required property real level\n}\n",
            ),
            (
                "Blocks/Card.qml",
                "import QtQuick\nBase {\n    count: 2\n    property real level: 1\n    \
                 required property bool done\n}\n",
            ),
            (SAMPLES, r#"{"Blocks/Card.qml": {"title": "Read"}}"#),
        ]);
        let sets = required_sets(&store, "Blocks/Card.qml", "CardShell.qml");
        let named = |name: &str| {
            sets.iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(named("done"), Some("false"));
        // The base's, with the block's sample.
        assert_eq!(named("title"), Some("\"Read\""));
        // Bound or declared again by the block: set already.
        assert_eq!(named("count"), None);
        assert_eq!(named("level"), None);
        let h = harness(&store, "Blocks/Card.qml").unwrap();
        assert!(h.text.contains("title: \"Read\"\n"), "{}", h.text);
    }

    #[test]
    fn a_delegate_sized_by_its_view_gets_a_size() {
        let store = store(&[(
            "Row.qml",
            "import QtQuick\nItem {\n    width: ListView.view.width\n    height: 40\n    \
             required property string name\n}\n",
        )]);
        let h = harness(&store, "Row.qml").unwrap();
        assert!(h.text.contains("width: 320"));
        assert!(!h.text.contains("height: 72"));
    }

    #[test]
    fn a_document_named_like_a_quick_type_is_the_one_made() {
        let store = store(&[(
            "Row.qml",
            "import QtQuick\nItem {\n    required property string name\n}\n",
        )]);
        let h = harness(&store, "Row.qml").unwrap();
        assert!(h.text.contains("import QtQuick as StudioQuick\n"));
        assert!(!h.text.contains("import QtQuick\n"));
        assert!(h.text.contains("StudioQuick.Item {"));
        assert!(h.text.contains("    Row {\n        id: subject\n"));
        assert!(outline::outline(&h.text).is_some());
    }

    #[test]
    fn a_store_shows_what_it_offers() {
        let store = store(&[
            (
                "Blocks/qmldir",
                "module Blocks\n# block HabitStore 1.0 HabitStore.qml\n",
            ),
            (
                "Blocks/HabitStore.qml",
                "import QtQuick\nItem {\n    property alias habits: model\n    \
             signal added(string title)\n    ListModel { id: model }\n    \
             function add(title) { model.append({ title: title }) }\n}\n",
            ),
        ]);
        let h = harness(&store, "Blocks/HabitStore.qml").unwrap();
        assert!(h.text.contains("HabitStore {\n        id: subject\n    }"));
        assert!(h.text.contains("root.shown(subject.habits)"));
        assert!(h.text.contains("\"add(title)\""));
        assert!(h.text.contains("\"added(string title)\""));
        assert!(outline::outline(&h.text).is_some());
        let line = instance_line(&h.text).unwrap();
        assert_eq!(
            h.text.lines().nth(line - 1).map(str::trim),
            Some("HabitStore {")
        );
    }

    #[test]
    fn a_block_with_a_size_of_its_own_stands_at_it() {
        let store = store(&[
            (
                "Blocks/qmldir",
                "module Blocks\n# block Badge 1.0 Badge.qml\n",
            ),
            (
                "Blocks/Badge.qml",
                "import QtQuick\nRectangle {\n    width: 200\n    height: 100\n}\n",
            ),
        ]);
        let h = harness(&store, "Blocks/Badge.qml").unwrap();
        assert!(
            h.text
                .contains("Badge {\n        id: subject\n        anchors.centerIn: parent\n    }")
        );
        assert!(!h.text.contains("width: 320"));
        assert!(!h.text.contains("height: 72"));
    }

    #[test]
    fn what_shows_itself_has_no_harness() {
        let store = store(&[
            ("Main.qml", "import QtQuick\nItem {\n    Rectangle {}\n}\n"),
            (
                "Win.qml",
                "import QtQuick\nWindow {\n    required property int n\n}\n",
            ),
            (
                "lower.qml",
                "import QtQuick\nItem {\n    required property int n\n}\n",
            ),
            ("Empty.qml", "import QtQuick\nItem {}\n"),
            (
                "Holds.qml",
                "import QtQuick\nItem {\n    property int n\n}\n",
            ),
        ]);
        for doc in ["Main.qml", "Win.qml", "lower.qml", "Empty.qml", "Holds.qml"] {
            assert_eq!(harness(&store, doc), None, "{doc}");
        }
    }

    #[test]
    fn only_studios_own_file_is_replaced() {
        let card = "import QtQuick\nItem {\n    required property int n\n}\n";
        let own = store(&[("Card.qml", card)]);
        let h = harness(&own, "Card.qml").unwrap();
        assert!(h.text.starts_with(MARK));
        own.write(&h.path, &h.text).unwrap();
        assert_eq!(harness(&own, "Card.qml"), Some(h));
        let theirs = store(&[("Card.qml", card), (".Card.preview.qml", "// Mine\n")]);
        assert_eq!(harness(&theirs, "Card.qml"), None);
    }

    #[test]
    fn a_standing_harness_takes_its_file_along() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".Card.preview.qml");
        std::fs::write(&file, MARK).unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        drop(h.standing(Some(dir.path())).expect("held"));
        assert!(!file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_standing_harness_leaves_what_a_linked_directory_points_at() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(project.join("cards")).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        let h = Harness {
            path: "cards/.Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        let standing = h.standing(Some(&project)).expect("held");
        // Swapped for a link to a marked file of another project. The
        // directory holds this preview's hold, so it goes with it.
        std::fs::remove_dir_all(project.join("cards")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, project.join("cards")).unwrap();
        let theirs = elsewhere.join(".Card.preview.qml");
        std::fs::write(&theirs, MARK).unwrap();
        drop(standing);
        assert!(theirs.exists());
    }

    #[test]
    fn a_standing_harness_leaves_a_persons_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".Card.preview.qml");
        std::fs::write(&file, "// Mine\n").unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        drop(h.standing(Some(dir.path())).expect("held"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "// Mine\n");
    }

    #[test]
    fn a_harness_another_studio_previews_from_stays_until_it_ends() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".Card.preview.qml");
        std::fs::write(&file, MARK).unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        // Two Studios on one project, each previewing the component.
        let first = h.standing(Some(dir.path())).expect("held");
        let mut second = h.standing(Some(dir.path())).expect("held");
        drop(first);
        assert!(file.exists(), "removed while another preview ran from it");
        second.leave(&studio_core::FsStore::new(dir.path()).unwrap());
        assert!(!file.exists());
        drop(second);
        // Nothing of either preview is left in the project but the gate.
        assert_eq!(left(dir.path()), [".studio"]);
    }

    #[test]
    fn a_killed_studios_hold_keeps_no_harness() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".Card.preview.qml");
        std::fs::write(&file, MARK).unwrap();
        // What a killed Studio leaves: its hold, no longer locked.
        std::fs::write(dir.path().join(".Card.preview.qml.1-0-0.lock"), "").unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        drop(h.standing(Some(dir.path())).expect("held"));
        assert_eq!(left(dir.path()), [".studio"]);
    }

    /// What is in `dir`, by name.
    fn left(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    /// Takes the project's preview gate, as another Studio would.
    fn take_gate(dir: &Path) -> File {
        std::fs::create_dir_all(dir.join(".studio")).unwrap();
        let gate = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(".studio/preview.lock"))
            .unwrap();
        gate.lock().unwrap();
        gate
    }

    #[test]
    fn a_harness_is_counted_and_removed_while_no_other_studio_makes_its_hold() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".Card.preview.qml");
        std::fs::write(&file, MARK).unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        let first = h.standing(Some(dir.path())).expect("held");
        // Another Studio starts a preview of the component: it makes its
        // hold under the gate, and writes the file after.
        let gate = take_gate(dir.path());
        let (ended, end) = std::sync::mpsc::channel();
        let ending = std::thread::spawn(move || {
            drop(first);
            ended.send(()).unwrap();
        });
        assert!(
            end.recv_timeout(std::time::Duration::from_millis(200))
                .is_err(),
            "the holds were counted while another Studio was making its own"
        );
        let theirs = File::create(dir.path().join(".Card.preview.qml.other-0.lock")).unwrap();
        theirs.try_lock().unwrap();
        gate.unlock().unwrap();
        ending.join().unwrap();
        assert!(file.exists(), "removed under the other Studio's preview");
    }

    #[test]
    fn a_hold_is_made_while_no_other_studio_counts_them() {
        let dir = tempfile::tempdir().unwrap();
        let gate = take_gate(dir.path());
        let (made, make) = std::sync::mpsc::channel();
        let project = dir.path().to_owned();
        let making = std::thread::spawn(move || {
            let h = Harness {
                path: ".Card.preview.qml".to_owned(),
                text: MARK.to_owned(),
            };
            let standing = h.standing(Some(&project)).expect("held");
            made.send(()).unwrap();
            standing
        });
        assert!(
            make.recv_timeout(std::time::Duration::from_millis(200))
                .is_err(),
            "a hold was made while another Studio counted them"
        );
        gate.unlock().unwrap();
        let _standing = making.join().unwrap();
        // Made in place, under no other name: one a killed Studio half made
        // is a lock file nobody holds, which the next count removes.
        let holds: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".Card.preview.qml."))
            .collect();
        assert_eq!(holds.len(), 1, "{holds:?}");
        assert!(holds[0].ends_with(".lock"), "{holds:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_gate_leads_nowhere_out_of_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(project.join(".studio")).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        // Planted by whoever else writes to the project: a link to a file
        // that is not there yet.
        std::os::unix::fs::symlink(elsewhere.join("made"), project.join(".studio/preview.lock"))
            .unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        // Without the gate nothing is held, so nothing is written either.
        assert!(h.standing(Some(&project)).is_none());
        assert!(
            !elsewhere.join("made").exists(),
            "a file was made through the link"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_hold_is_never_made_through_a_linked_directory() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, project.join("cards")).unwrap();
        let h = Harness {
            path: "cards/.Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        let standing = h.standing(Some(&project));
        assert!(standing.is_none());
        assert_eq!(left(&elsewhere), Vec::<String>::new());
        drop(standing);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_by_the_harness_name_is_not_studios_to_remove() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        let theirs = elsewhere.join("Theirs.qml");
        std::fs::write(&theirs, MARK).unwrap();
        let link = project.join(".Card.preview.qml");
        std::os::unix::fs::symlink(&theirs, &link).unwrap();
        let h = Harness {
            path: ".Card.preview.qml".to_owned(),
            text: MARK.to_owned(),
        };
        drop(h.standing(Some(&project)).expect("held"));
        assert!(link.symlink_metadata().is_ok(), "the link was removed");
        assert!(theirs.exists());
    }

    #[test]
    fn only_studios_stand_in_is_no_document() {
        let store = store(&[
            (".Card.preview.qml", MARK),
            ("b/.Card.preview.qml", "// Mine\n"),
            (".hidden.qml", MARK),
        ]);
        assert!(stands_in(&store, ".Card.preview.qml"));
        assert!(!stands_in(&store, "b/.Card.preview.qml"));
        assert!(!stands_in(&store, ".hidden.qml"));
    }

    #[test]
    fn without_a_directory_the_store_takes_the_file_out() {
        let store = store(&[("Own.qml", MARK), ("Theirs.qml", "// Mine\n")]);
        for path in ["Own.qml", "Theirs.qml"] {
            let h = Harness {
                path: path.to_owned(),
                text: MARK.to_owned(),
            };
            h.standing(None)
                .expect("no directory to hold in")
                .leave(&store);
        }
        assert!(!store.exists("Own.qml"));
        assert!(store.exists("Theirs.qml"));
    }

    #[test]
    fn names_are_titled() {
        assert_eq!(titled("dueDate"), "Due date");
        assert_eq!(titled("title"), "Title");
    }
}
