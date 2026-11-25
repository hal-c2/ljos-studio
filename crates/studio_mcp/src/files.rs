//! The project's files that are not documents: a README, a script a document
//! imports, a style sheet.
//!
//! A document is saved as it is edited and has a history. Any other file is
//! held here as it is edited and written only when saved, as a text editor
//! does: so a save is the one moment it changes on disk, and what another
//! program saved meanwhile is seen before it is written over.

use std::collections::BTreeMap;

use ljos_mcp_core::{ErrorCode, SurfaceError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use studio_core::{Store, StoreError, document_path, inside, listed};

use crate::{Studio, store_error};

/// Far above a file a person edits by hand; a mistaken path (a database, a
/// log) stops here, before it is read into memory.
const MOST_BYTES: usize = 8 * 1024 * 1024;
/// A listing stops here, so a folder of generated files cannot fill every
/// answer and every view; `more` says it did.
pub(crate) const MOST_FILES: usize = 5000;

#[derive(Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FilesAction {
    List,
    Read,
    Edit,
    Save,
    SaveAll,
    Revert,
    Close,
    New,
    Rename,
    Remove,
}

impl FilesAction {
    /// Only listing reads nothing but names.
    pub(crate) fn needs_fs(self) -> bool {
        self != Self::List
    }
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct FileInfo {
    pub(crate) path: String,
    /// What the file holds, with the edits held; left out after `close`,
    /// `remove` and a rename.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    /// Edits are held that are not on disk.
    pub(crate) dirty: bool,
    /// The file on disk changed since it was read, under edits held.
    pub(crate) outside: bool,
}

/// A file being edited.
#[derive(Debug, Default)]
pub(crate) struct Held {
    /// What the disk held when it was read or last saved.
    base: String,
    /// The edits, when there are any.
    text: Option<String>,
    /// One more each time `base` is taken again from the disk, so a window
    /// showing the file knows to read it again.
    pub(crate) stamp: u64,
    /// What the disk holds now differs from `base`.
    pub(crate) outside: bool,
}

impl Held {
    pub(crate) fn dirty(&self) -> bool {
        self.text.as_ref().is_some_and(|t| *t != self.base)
    }

    fn text(&self) -> &str {
        self.text.as_deref().unwrap_or(&self.base)
    }
}

/// The files held open, by path.
pub(crate) type Open = BTreeMap<String, Held>;

/// What the files actions answer, beside the project.
#[derive(Default)]
pub(crate) struct Answer {
    pub(crate) files: Option<Vec<String>>,
    pub(crate) more: bool,
    pub(crate) file: Option<FileInfo>,
    pub(crate) saved: Option<Vec<String>>,
}

fn kind(code: ErrorCode, kind: &str, message: String) -> SurfaceError {
    SurfaceError::new(code, message).with_details(json!({ "kind": kind }))
}

fn needs_file(file: Option<&str>, action: &str) -> Result<String, SurfaceError> {
    file.map(str::to_owned)
        .ok_or_else(|| SurfaceError::invalid_argument(format!("`{action}` needs `file`")))
}

/// `path` as a file of the project a person edits: a plain path, outside the
/// directories a walk leaves out, judged where it really is. With it, whether
/// it is a document, also judged where it really is: a link named `notes.txt`
/// to `Main.qml` is edited as the document it is.
fn checked(store: &dyn Store, path: &str) -> Result<(String, bool), SurfaceError> {
    let denied = || {
        SurfaceError::new(
            ErrorCode::PathDenied,
            format!("`{path}` is not a file of the project"),
        )
    };
    if !inside(path) || !listed(path) || shown(path).is_none() {
        return Err(denied());
    }
    let real = store.resolved(path).map_err(store_error)?;
    if !listed(&real) {
        return Err(denied());
    }
    Ok((path.to_owned(), document_path(path) || document_path(&real)))
}

/// Whether a listing has `path`: not a file Studio stands in with, nor one it
/// is writing.
fn shown(path: &str) -> Option<()> {
    let name = path.rsplit('/').next()?;
    let standin = name.starts_with('.') && name.ends_with(".qml");
    (!standin && !name.ends_with(".studio-tmp")).then_some(())
}

/// The project's files, sorted, and whether there were more than a listing
/// holds.
pub(crate) fn list(store: &dyn Store) -> (Vec<String>, bool) {
    let mut all = store.list("");
    all.retain(|p| listed(p) && shown(p).is_some());
    let more = all.len() > MOST_FILES;
    all.truncate(MOST_FILES);
    (all, more)
}

fn read_disk(store: &dyn Store, path: &str) -> Result<String, SurfaceError> {
    let (size, _) = store.peek(path, 0).map_err(store_error)?;
    if size > MOST_BYTES as u64 {
        return Err(kind(
            ErrorCode::InvalidArgument,
            "too_large",
            format!("`{path}` is {size} bytes; a file opened as text is at most {MOST_BYTES}"),
        ));
    }
    let bytes = store.read_bytes(path).map_err(store_error)?;
    String::from_utf8(bytes).map_err(|_| {
        kind(
            ErrorCode::InvalidArgument,
            "not_text",
            format!("`{path}` is not UTF-8 text"),
        )
    })
}

/// Compares a held file with the disk. With no edits held it takes what the
/// disk says; under edits it notes that the disk moved.
pub(crate) fn look(store: &dyn Store, path: &str, held: &mut Held) {
    let Ok(disk) = read_disk(store, path) else {
        // Gone or unreadable: under edits that is a change; without, the
        // window keeps what it showed until it is closed.
        held.outside = held.dirty();
        return;
    };
    if disk == held.base {
        held.outside = false;
    } else if held.dirty() {
        held.outside = true;
    } else {
        held.base = disk;
        held.text = None;
        held.stamp += 1;
        held.outside = false;
    }
}

fn info(path: &str, held: &Held) -> FileInfo {
    FileInfo {
        path: path.to_owned(),
        text: Some(held.text().to_owned()),
        dirty: held.dirty(),
        outside: held.outside,
    }
}

fn gone(path: &str) -> FileInfo {
    FileInfo {
        path: path.to_owned(),
        text: None,
        dirty: false,
        outside: false,
    }
}

/// The files held with edits, sorted.
pub(crate) fn unsaved(open: &Open) -> Vec<String> {
    open.iter()
        .filter(|(_, h)| h.dirty())
        .map(|(p, _)| p.clone())
        .collect()
}

/// Refused while edits are held, unless `force` lets them go.
pub(crate) fn may_leave(open: &Open, force: bool) -> Result<(), SurfaceError> {
    let unsaved = unsaved(open);
    if unsaved.is_empty() || force {
        return Ok(());
    }
    Err(unsaved_error(unsaved))
}

fn unsaved_error(files: Vec<String>) -> SurfaceError {
    SurfaceError::invalid_argument(format!(
        "edits of {} are not saved: save them, or pass `force` to let them go",
        files.join(", ")
    ))
    .with_details(json!({ "kind": "unsaved", "files": files }))
}

fn document(path: &str, action: &str) -> SurfaceError {
    kind(
        ErrorCode::InvalidArgument,
        "document",
        format!(
            "`{path}` is a document, saved as it is edited: `{action}` it with the document tools"
        ),
    )
}

fn write(store: &dyn Store, path: &str, text: &str) -> Result<(), SurfaceError> {
    store.write(path, text).map_err(store_error)
}

fn save(store: &dyn Store, path: &str, held: &mut Held, force: bool) -> Result<(), SurfaceError> {
    look(store, path, held);
    if held.outside && !force {
        return Err(SurfaceError::new(
            ErrorCode::StaleRef,
            format!(
                "`{path}` changed on disk since it was read: read it again, or pass `force` to write over it"
            ),
        )
        .with_details(json!({ "kind": "changed_outside", "file": path })));
    }
    // Taken only once it is on disk: a write that fails keeps the edits.
    if let Some(text) = &held.text {
        write(store, path, text)?;
        held.base = held.text.take().expect("just written");
    }
    held.outside = false;
    Ok(())
}

/// The arguments the files actions take.
pub(crate) struct Asked<'a> {
    pub(crate) file: Option<&'a str>,
    pub(crate) text: Option<&'a str>,
    pub(crate) to: Option<&'a str>,
    pub(crate) force: bool,
}

pub(crate) fn act(
    studio: &mut Studio,
    action: FilesAction,
    a: &Asked<'_>,
) -> Result<Answer, SurfaceError> {
    let mut out = Answer::default();
    if !matches!(
        action,
        FilesAction::Edit | FilesAction::Read | FilesAction::List
    ) {
        studio.relist();
    }
    let name = match action {
        FilesAction::List => {
            let (files, more) = list(studio.session.store());
            out.files = Some(files);
            out.more = more;
            return Ok(out);
        }
        FilesAction::SaveAll => {
            let store = studio.session.store();
            // Every file is compared with the disk before any is written: a
            // refusal leaves all of them as they were, and names them all.
            let mut changed = Vec::new();
            for (path, held) in &mut studio.files {
                if held.dirty() {
                    look(store, path, held);
                    if held.outside {
                        changed.push(path.clone());
                    }
                }
            }
            if !changed.is_empty() && !a.force {
                studio.files_changed();
                return Err(SurfaceError::new(
                    ErrorCode::StaleRef,
                    format!(
                        "{} changed on disk since read: read them again, or pass `force` to write over them; nothing was saved",
                        changed.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")
                    ),
                )
                .with_details(json!({ "kind": "changed_outside", "files": changed, "saved": [] })));
            }
            let mut saved = Vec::new();
            for (path, held) in &mut studio.files {
                if !held.dirty() {
                    continue;
                }
                if let Err(error) = save(store, path, held, a.force) {
                    // A write the disk refused stops the rest; who asked
                    // hears which were saved before it.
                    let details = json!({ "file": path, "saved": saved });
                    studio.files_changed();
                    return Err(error.with_details(details));
                }
                saved.push(path.clone());
            }
            out.saved = Some(saved);
            studio.files_changed();
            return Ok(out);
        }
        FilesAction::Read => "read",
        FilesAction::Edit => "edit",
        FilesAction::Save => "save",
        FilesAction::Revert => "revert",
        FilesAction::Close => "close",
        FilesAction::New => "new",
        FilesAction::Rename => "rename",
        FilesAction::Remove => "remove",
    };
    let store = studio.session.store();
    let (path, is_document) = checked(store, &needs_file(a.file, name)?)?;
    out.file = Some(match action {
        FilesAction::Read if is_document => {
            let real = store.resolved(&path).map_err(store_error)?;
            let (text, _) = studio.session.text(&real).map_err(crate::error)?;
            FileInfo {
                path: path.clone(),
                text: Some(text.to_owned()),
                dirty: false,
                outside: false,
            }
        }
        FilesAction::Edit | FilesAction::Save | FilesAction::Revert | FilesAction::New
            if is_document =>
        {
            return Err(document(&path, name));
        }
        FilesAction::Read => {
            let held = match studio.files.get_mut(&path) {
                Some(held) => {
                    look(store, &path, held);
                    held
                }
                None => {
                    let base = read_disk(store, &path)?;
                    studio.files.entry(path.clone()).or_insert(Held {
                        base,
                        ..Held::default()
                    })
                }
            };
            info(&path, held)
        }
        FilesAction::Edit => {
            let text = a
                .text
                .ok_or_else(|| SurfaceError::invalid_argument("`edit` needs `text`".to_owned()))?;
            if !studio.files.contains_key(&path) {
                let base = read_disk(store, &path)?;
                studio.files.insert(
                    path.clone(),
                    Held {
                        base,
                        ..Held::default()
                    },
                );
            }
            let held = studio.files.get_mut(&path).expect("just held");
            // Edits taken back by typing leave a file that follows the disk
            // again, which only a look at it says: the next view takes one.
            let clean_again = held.dirty() && text == held.base;
            held.text = (text != held.base).then(|| text.to_owned());
            let shown = info(&path, held);
            if clean_again {
                studio.relist();
            }
            shown
        }
        FilesAction::Save => {
            let held = studio.files.get_mut(&path).ok_or_else(|| not_open(&path))?;
            save(store, &path, held, a.force)?;
            info(&path, held)
        }
        FilesAction::Revert => {
            let held = studio.files.get_mut(&path).ok_or_else(|| not_open(&path))?;
            // Read first: a file that is gone or no longer text keeps its edits.
            let base = read_disk(store, &path)?;
            held.text = None;
            held.base = base;
            held.stamp += 1;
            held.outside = false;
            info(&path, held)
        }
        FilesAction::Close => {
            if studio.files.get(&path).is_some_and(Held::dirty) && !a.force {
                return Err(unsaved_error(vec![path]));
            }
            studio.files.remove(&path);
            gone(&path)
        }
        FilesAction::New => {
            if store.exists(&path) {
                return Err(exists(&path));
            }
            held_dirty(&studio.files, &path)?;
            let text = a.text.unwrap_or_default();
            write(store, &path, text)?;
            studio.files.insert(
                path.clone(),
                Held {
                    base: text.to_owned(),
                    ..Held::default()
                },
            );
            info(&path, &studio.files[&path])
        }
        FilesAction::Rename => {
            let (to, to_document) = checked(
                store,
                a.to.ok_or_else(|| {
                    SurfaceError::invalid_argument("`rename` needs `to`".to_owned())
                })?,
            )?;
            if store.exists(&to) {
                return Err(exists(&to));
            }
            held_dirty(&studio.files, &to)?;
            // A document is edited by the document tools, so edits held
            // under another name are saved or let go before it becomes one.
            if to_document && studio.files.get(&path).is_some_and(Held::dirty) {
                return Err(unsaved_error(vec![path]));
            }
            if !store.exists(&path) {
                return Err(store_error(StoreError::Missing(path)));
            }
            store.rename(&path, &to).map_err(store_error)?;
            if let Some(held) = studio.files.remove(&path)
                && !to_document
            {
                studio.files.insert(to.clone(), held);
            }
            match studio.files.get(&to) {
                Some(held) => info(&to, held),
                None => gone(&to),
            }
        }
        FilesAction::Remove => {
            store.remove(&path).map_err(store_error)?;
            studio.files.remove(&path);
            gone(&path)
        }
        FilesAction::List | FilesAction::SaveAll => unreachable!("answered above"),
    });
    studio.files_changed();
    Ok(out)
}

/// Refused while `path` holds edits, though its file may be gone from disk:
/// a new file or a rename there would put them out of reach.
fn held_dirty(open: &Open, path: &str) -> Result<(), SurfaceError> {
    match open.get(path) {
        Some(held) if held.dirty() => Err(unsaved_error(vec![path.to_owned()])),
        _ => Ok(()),
    }
}

fn exists(path: &str) -> SurfaceError {
    kind(
        ErrorCode::InvalidArgument,
        "exists",
        format!("`{path}` exists"),
    )
}

fn not_open(path: &str) -> SurfaceError {
    kind(
        ErrorCode::InvalidArgument,
        "not_open",
        format!("`{path}` is not open: `read` or `edit` it first"),
    )
}
