//! A project held in the browser's storage.
//!
//! Browser storage answers later (OPFS, IndexedDB: promises), while
//! [`Store`] answers now, as the session needs: an edit reads the text it
//! changes and writes the result in one step. So the project is loaded
//! whole when it opens and kept here, reads are answered from that copy,
//! and each write replaces the file here at once and is queued for storage.
//! The page drains the queue after every tool call ([`WebStore::take`]) and
//! answers the call only once storage has the files, so what a tool's
//! answer says is stored is stored.
//!
//! Against `FsStore`: a write is whole-file replace in both, and neither
//! leaves half a file (OPFS's `createWritable` commits on `close`, as a
//! rename does). What differs: a write reaches storage after `write`
//! returns, not before, so a page closed in between loses it; and a write
//! storage refuses is known only then, so the call that made it is
//! refused after its edit is in the session (its answer says which file).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

use studio_core::{Store, StoreError, inside, listed};

/// What storage is to do with one file.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pending {
    pub path: String,
    /// The file's whole text, or `None` to remove it.
    pub text: Option<String>,
}

#[derive(Debug, Default)]
struct Held {
    files: BTreeMap<String, String>,
    /// The last thing to do per file: a later write replaces an earlier
    /// one not yet stored, so storage sees each file's last text once.
    queued: BTreeMap<String, Option<String>>,
}

/// The project's files, shared by the session and whoever stores them.
#[derive(Clone, Debug, Default)]
pub struct WebStore {
    held: Arc<Mutex<Held>>,
    place: Arc<str>,
}

impl WebStore {
    /// A project with `files` as storage has them, called `place` (what a
    /// person is shown: an OPFS directory's name, `memory`).
    pub fn new(place: &str, files: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            held: Arc::new(Mutex::new(Held {
                files: files.into_iter().collect(),
                queued: BTreeMap::new(),
            })),
            place: place.into(),
        }
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// What storage has yet to do, oldest file first; the queue is then
    /// empty.
    pub fn take(&self) -> Vec<Pending> {
        std::mem::take(&mut self.held().queued)
            .into_iter()
            .map(|(path, text)| Pending { path, text })
            .collect()
    }

    /// Storage did not do `pending`: the file goes again with the next
    /// take, as the session holds it now. Not `pending`'s own text: a newer
    /// one may have been taken by a call storing alongside, and stored
    /// already, and the older text must not land after it.
    pub fn unstored(&self, pending: Pending) {
        let mut held = self.held();
        let now = held.files.get(&pending.path).cloned();
        held.queued.entry(pending.path).or_insert(now);
    }

    /// Storage has `pending` from somewhere other than this session (another
    /// tab, a file a person dropped in): reads see it from now on, as
    /// `FsStore`'s see a file written into the directory. Nothing is queued,
    /// since storage has it, and a write of the file still queued is older
    /// and is dropped. A document the session has open keeps its text, as
    /// it does natively.
    pub fn stored(&self, pending: Pending) -> Result<(), StoreError> {
        Self::checked(&pending.path)?;
        let mut held = self.held();
        held.queued.remove(&pending.path);
        match pending.text {
            Some(text) => held.files.insert(pending.path, text),
            None => held.files.remove(&pending.path),
        };
        Ok(())
    }

    /// Every file and its text, as a preview is given them.
    pub fn files(&self) -> BTreeMap<String, String> {
        self.held().files.clone()
    }

    fn checked(path: &str) -> Result<(), StoreError> {
        if inside(path) {
            Ok(())
        } else {
            Err(StoreError::Outside(path.to_owned()))
        }
    }
}

/// How a file that is not text is held: browser storage and what is handed to
/// a preview are text, so a picture goes as a data URL's text.
const BYTES: &str = "data:application/octet-stream;base64,";

impl Store for WebStore {
    fn list(&self, extension: &str) -> Vec<String> {
        // `BTreeMap` keeps them sorted, as a directory walk sorts them.
        self.held()
            .files
            .keys()
            .filter(|p| p.ends_with(extension) && listed(p))
            .cloned()
            .collect()
    }

    fn read(&self, path: &str) -> Result<String, StoreError> {
        Self::checked(path)?;
        self.held()
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| StoreError::Missing(path.to_owned()))
    }

    fn write(&self, path: &str, text: &str) -> Result<(), StoreError> {
        Self::checked(path)?;
        let mut held = self.held();
        held.files.insert(path.to_owned(), text.to_owned());
        held.queued.insert(path.to_owned(), Some(text.to_owned()));
        Ok(())
    }

    fn remove(&self, path: &str) -> Result<(), StoreError> {
        Self::checked(path)?;
        let mut held = self.held();
        held.files
            .remove(path)
            .ok_or_else(|| StoreError::Missing(path.to_owned()))?;
        held.queued.insert(path.to_owned(), None);
        Ok(())
    }

    fn exists(&self, path: &str) -> bool {
        inside(path) && self.held().files.contains_key(path)
    }

    fn read_bytes(&self, path: &str) -> Result<Vec<u8>, StoreError> {
        let text = self.read(path)?;
        match text.strip_prefix(BYTES) {
            Some(data) => STANDARD.decode(data).map_err(|e| StoreError::Io {
                path: path.to_owned(),
                message: e.to_string(),
            }),
            None => Ok(text.into_bytes()),
        }
    }

    fn write_bytes(&self, path: &str, bytes: &[u8]) -> Result<(), StoreError> {
        match std::str::from_utf8(bytes) {
            Ok(text) => self.write(path, text),
            Err(_) => self.write(path, &format!("{BYTES}{}", STANDARD.encode(bytes))),
        }
    }

    fn describe(&self) -> String {
        self.place.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> WebStore {
        WebStore::new(
            "memory",
            [
                ("b/Two.qml".to_owned(), "Item {}\n".to_owned()),
                ("One.qml".to_owned(), "Item {}\n".to_owned()),
                (".studio/Hidden.qml".to_owned(), "Item {}\n".to_owned()),
                ("node_modules/x/Dep.qml".to_owned(), "Item {}\n".to_owned()),
            ],
        )
    }

    #[test]
    fn files_are_listed_as_a_directory_lists_them() {
        assert_eq!(store().list(".qml"), ["One.qml", "b/Two.qml"]);
    }

    #[test]
    fn a_path_cannot_leave_the_project() {
        let store = store();
        for bad in [
            "../x.qml",
            "/etc/passwd",
            "",
            "a/../../b.qml",
            "a//b.qml",
            "C:x.qml",
        ] {
            assert!(
                matches!(store.read(bad), Err(StoreError::Outside(_))),
                "{bad}"
            );
            assert!(
                matches!(store.write(bad, ""), Err(StoreError::Outside(_))),
                "{bad}"
            );
        }
        assert!(store.take().is_empty());
    }

    #[test]
    fn storage_gets_each_files_last_text_once() {
        let store = store();
        store.write("One.qml", "Rectangle {}\n").unwrap();
        store.write("New.qml", "Text {}\n").unwrap();
        store.write("One.qml", "Text {}\n").unwrap();
        store.remove("b/Two.qml").unwrap();
        assert_eq!(store.read("One.qml").unwrap(), "Text {}\n");
        assert_eq!(
            store.take(),
            [
                Pending {
                    path: "New.qml".into(),
                    text: Some("Text {}\n".into())
                },
                Pending {
                    path: "One.qml".into(),
                    text: Some("Text {}\n".into())
                },
                Pending {
                    path: "b/Two.qml".into(),
                    text: None
                },
            ]
        );
        assert!(store.take().is_empty());
    }

    #[test]
    fn a_write_storage_refused_goes_again_unless_a_newer_one_came() {
        let store = store();
        store.write("One.qml", "A {}\n").unwrap();
        store.write("b/Two.qml", "B {}\n").unwrap();
        let [one, two] = <[Pending; 2]>::try_from(store.take()).unwrap();
        store.write("b/Two.qml", "C {}\n").unwrap();
        store.unstored(one.clone());
        store.unstored(two);
        assert_eq!(
            store.take(),
            [
                one,
                Pending {
                    path: "b/Two.qml".into(),
                    text: Some("C {}\n".into())
                },
            ]
        );
    }

    /// Two calls store at once: the later one's text was taken, and may be
    /// stored already, when the earlier one's write fails. Storing the
    /// failed text again would put the older file back over the newer.
    #[test]
    fn a_refused_write_goes_again_as_the_newest_text_even_when_that_was_taken() {
        let store = store();
        store.write("One.qml", "A {}\n").unwrap();
        let [first] = <[Pending; 1]>::try_from(store.take()).unwrap();
        store.write("One.qml", "B {}\n").unwrap();
        let [_second] = <[Pending; 1]>::try_from(store.take()).unwrap();
        store.unstored(first);
        assert_eq!(
            store.take(),
            [Pending {
                path: "One.qml".into(),
                text: Some("B {}\n".into())
            }]
        );
    }

    #[test]
    fn what_storage_got_elsewhere_is_read_and_not_stored_again() {
        let store = store();
        store.write("One.qml", "A {}\n").unwrap();
        store
            .stored(Pending {
                path: "One.qml".into(),
                text: Some("B {}\n".into()),
            })
            .unwrap();
        store
            .stored(Pending {
                path: "New.qml".into(),
                text: Some("C {}\n".into()),
            })
            .unwrap();
        store
            .stored(Pending {
                path: "b/Two.qml".into(),
                text: None,
            })
            .unwrap();
        assert_eq!(store.read("One.qml").unwrap(), "B {}\n");
        assert_eq!(store.list(".qml"), ["New.qml", "One.qml"]);
        assert!(store.take().is_empty());
        assert!(matches!(
            store.stored(Pending {
                path: "../x.qml".into(),
                text: None
            }),
            Err(StoreError::Outside(_))
        ));
    }

    #[test]
    fn removing_what_is_not_there_is_refused() {
        let store = store();
        assert!(matches!(
            store.remove("Nope.qml"),
            Err(StoreError::Missing(_))
        ));
        assert!(store.take().is_empty());
    }
}
