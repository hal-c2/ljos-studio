//! Where documents are read from and written to.
//!
//! The filesystem natively, browser storage on the web, memory in tests:
//! the session only ever sees this trait, so the same session runs on all
//! three.

use std::collections::BTreeMap;
#[cfg(feature = "fs")]
use std::io::Write as _;
#[cfg(feature = "fs")]
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("`{0}` is outside the project")]
    Outside(String),
    #[error("`{0}` does not exist")]
    Missing(String),
    #[error("`{0}` is not a document of this project")]
    NotDocument(String),
    #[error("`{path}`: {message}")]
    Io { path: String, message: String },
}

/// A project's files, by paths relative to the project with `/` between
/// parts.
pub trait Store: Send + Sync {
    /// Every file below the project whose name ends in `extension`, sorted.
    fn list(&self, extension: &str) -> Vec<String>;
    fn read(&self, path: &str) -> Result<String, StoreError>;
    fn write(&self, path: &str, text: &str) -> Result<(), StoreError>;
    fn remove(&self, path: &str) -> Result<(), StoreError>;
    fn exists(&self, path: &str) -> bool;
    /// What to show a person as the project's place.
    fn describe(&self) -> String;
    /// Where `path` really is, relative to the project, with links followed.
    fn resolved(&self, path: &str) -> Result<String, StoreError> {
        Ok(path.to_owned())
    }
}

/// A project directory on disk.
#[cfg(feature = "fs")]
#[derive(Debug)]
pub struct FsStore {
    root: PathBuf,
}

#[cfg(feature = "fs")]
impl FsStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root: PathBuf = root.into();
        let root = root.canonicalize().map_err(|e| StoreError::Io {
            path: root.display().to_string(),
            message: e.to_string(),
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn confine(&self, path: &str) -> Result<PathBuf, StoreError> {
        let relative = Path::new(path);
        // Only plain names: `..`, a root or a prefix could leave the project.
        if path.is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(StoreError::Outside(path.to_owned()));
        }
        let full = self.root.join(relative);
        // The part that exists is judged where it really is, so a link cannot
        // lead out, and the path written to is that place: a rename over a
        // link would replace the link and not the file it names.
        for at in full.ancestors() {
            if at.symlink_metadata().is_err() {
                continue;
            }
            let real = at
                .canonicalize()
                .map_err(|_| StoreError::Outside(path.to_owned()))?;
            if !real.starts_with(&self.root) {
                return Err(StoreError::Outside(path.to_owned()));
            }
            // Joining nothing would add a `/` and make a file a directory.
            let rest = full.strip_prefix(at).expect("one of its ancestors");
            return Ok(if rest.as_os_str().is_empty() {
                real
            } else {
                real.join(rest)
            });
        }
        Ok(full)
    }
}

/// Whether `path` can be a document: a `.qml` file or a theme file, in
/// normal form so one file is not two documents by spelling, outside the
/// directories a walk leaves out.
pub(crate) fn document_path(path: &str) -> bool {
    let mut parts = path.split('/').rev();
    let file = parts.next().unwrap_or_default();
    (file.ends_with(".qml") || file == crate::theme::FILE)
        && !matches!(file, "." | "..")
        && parts.all(|dir| !dir.is_empty() && dir != "." && dir != ".." && !skipped(dir))
}

/// Directories a walk leaves out: build output and tools' own.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "node_modules" | "build")
}

/// Whether a project's listing has the file at `path` (`/` between parts):
/// none below a directory a walk of the project leaves out. A store that
/// holds files by path (browser storage) lists what a directory would.
pub fn listed(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    !parts.into_iter().any(skipped)
}

/// Whether `path` names a place inside the project: plain names between
/// `/`, never `..`, `.`, a root or an empty part. A `\` or a `:` would be a
/// separator or a drive on some platform, so a project that leaves through
/// one there cannot be made anywhere.
pub fn inside(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|p| {
            !p.is_empty() && p != "." && p != ".." && !p.contains('\\') && !p.contains(':')
        })
}

#[cfg(feature = "fs")]
fn walk(dir: &Path, root: &Path, extension: &str, out: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        // `file_type` does not follow links, so a link cannot lead the walk
        // out of the project.
        match entry.file_type() {
            Ok(t) if t.is_dir() && !skipped(name) => walk(&path, root, extension, out),
            Ok(t) if t.is_file() && name.ends_with(extension) => {
                if let Ok(rel) = path.strip_prefix(root) {
                    let parts: Vec<_> = rel
                        .components()
                        .filter_map(|c| c.as_os_str().to_str())
                        .collect();
                    out.push(parts.join("/"));
                }
            }
            _ => {}
        }
    }
}

#[cfg(feature = "fs")]
impl Store for FsStore {
    fn list(&self, extension: &str) -> Vec<String> {
        let mut out = Vec::new();
        walk(&self.root, &self.root, extension, &mut out);
        out.sort();
        out
    }

    fn read(&self, path: &str) -> Result<String, StoreError> {
        let full = self.confine(path)?;
        std::fs::read_to_string(&full).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => StoreError::Missing(path.to_owned()),
            _ => StoreError::Io {
                path: path.to_owned(),
                message: e.to_string(),
            },
        })
    }

    fn write(&self, path: &str, text: &str) -> Result<(), StoreError> {
        let full = self.confine(path)?;
        let io = |e: std::io::Error| StoreError::Io {
            path: path.to_owned(),
            message: e.to_string(),
        };
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        // Written beside and renamed over, so a watcher (the preview's hot
        // reload) never reads half a file and a crash leaves the old text
        // or the new, never a mix. The price is the rename: ext4 has the
        // new file's data on its way to the disk before it replaces the
        // old one, and behind other writing to the same drive that takes
        // up to a second, which is the whole tail of an edit's way to the
        // preview (DESIGN.md section 6).
        let tmp = full.with_extension("qml.studio-tmp");
        // A link left at that name would take the text wherever it leads.
        match std::fs::remove_file(&tmp) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io(e)),
            _ => {}
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(io)?;
        std::fs::rename(&tmp, &full).map_err(io)
    }

    fn remove(&self, path: &str) -> Result<(), StoreError> {
        let full = self.confine(path)?;
        std::fs::remove_file(&full).map_err(|e| StoreError::Io {
            path: path.to_owned(),
            message: e.to_string(),
        })
    }

    fn exists(&self, path: &str) -> bool {
        self.confine(path).is_ok_and(|p| p.is_file())
    }

    fn describe(&self) -> String {
        self.root.display().to_string()
    }

    fn resolved(&self, path: &str) -> Result<String, StoreError> {
        let full = self.confine(path)?;
        let relative = full.strip_prefix(&self.root).unwrap_or(&full);
        relative
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| StoreError::Outside(path.to_owned()))
    }
}

/// Files kept in memory: tests, and a project that has not been saved.
#[derive(Debug, Default)]
pub struct MemStore {
    files: Mutex<BTreeMap<String, String>>,
}

impl MemStore {
    pub fn new(files: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            files: Mutex::new(files.into_iter().collect()),
        }
    }

    fn files(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, String>> {
        self.files.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Store for MemStore {
    fn list(&self, extension: &str) -> Vec<String> {
        self.files()
            .keys()
            .filter(|k| k.ends_with(extension))
            .cloned()
            .collect()
    }

    fn read(&self, path: &str) -> Result<String, StoreError> {
        self.files()
            .get(path)
            .cloned()
            .ok_or_else(|| StoreError::Missing(path.to_owned()))
    }

    fn write(&self, path: &str, text: &str) -> Result<(), StoreError> {
        self.files().insert(path.to_owned(), text.to_owned());
        Ok(())
    }

    fn remove(&self, path: &str) -> Result<(), StoreError> {
        self.files()
            .remove(path)
            .map(|_| ())
            .ok_or_else(|| StoreError::Missing(path.to_owned()))
    }

    fn exists(&self, path: &str) -> bool {
        self.files().contains_key(path)
    }

    fn describe(&self) -> String {
        "memory".to_owned()
    }
}

#[cfg(all(test, feature = "fs"))]
mod tests {
    use super::*;

    #[test]
    fn a_path_cannot_leave_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        for bad in ["../x.qml", "/etc/passwd", "", "a/../../b.qml"] {
            assert!(
                matches!(store.read(bad), Err(StoreError::Outside(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_link_that_stays_in_the_project_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        store.write("real/One.qml", "Item {}").unwrap();
        std::os::unix::fs::symlink("real", dir.path().join("alias")).unwrap();
        assert_eq!(store.read("alias/One.qml").unwrap(), "Item {}");
        store.write("alias/Two.qml", "Item {}").unwrap();
        assert!(store.exists("real/Two.qml"));
    }

    #[test]
    fn a_new_file_in_a_new_directory_is_inside() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        store.write("a/b/c/New.qml", "Item {}").unwrap();
        assert!(store.exists("a/b/c/New.qml"));
    }

    #[test]
    fn a_link_cannot_lead_out_of_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("Theirs.qml"), "theirs").unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        let root = dir.path();
        std::os::unix::fs::symlink(outside.path(), root.join("dir")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("Theirs.qml"), root.join("File.qml"))
            .unwrap();
        std::os::unix::fs::symlink(root.join("nowhere"), root.join("Dangling.qml")).unwrap();
        for bad in [
            "dir/Theirs.qml",
            "dir/New.qml",
            "dir/a/b/New.qml",
            "File.qml",
            "Dangling.qml",
        ] {
            assert!(
                matches!(store.read(bad), Err(StoreError::Outside(_))),
                "read {bad}"
            );
            assert!(
                matches!(store.write(bad, "mine"), Err(StoreError::Outside(_))),
                "write {bad}"
            );
            assert!(
                matches!(store.remove(bad), Err(StoreError::Outside(_))),
                "remove {bad}"
            );
            assert!(!store.exists(bad), "exists {bad}");
        }
        assert_eq!(
            std::fs::read_to_string(outside.path().join("Theirs.qml")).unwrap(),
            "theirs"
        );
        assert!(!outside.path().join("New.qml").exists());
    }

    #[test]
    fn a_write_through_a_link_inside_keeps_the_link() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        store.write("parts/Card.qml", "old").unwrap();
        std::os::unix::fs::symlink(
            dir.path().join("parts/Card.qml"),
            dir.path().join("Link.qml"),
        )
        .unwrap();
        store.write("Link.qml", "new").unwrap();
        assert!(
            dir.path()
                .join("Link.qml")
                .symlink_metadata()
                .unwrap()
                .is_symlink()
        );
        assert_eq!(store.read("parts/Card.qml").unwrap(), "new");
        assert_eq!(store.resolved("Link.qml").unwrap(), "parts/Card.qml");
    }

    #[test]
    fn a_link_left_at_the_temporary_name_is_not_written_through() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let theirs = outside.path().join("theirs");
        std::fs::write(&theirs, "theirs").unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        std::os::unix::fs::symlink(&theirs, dir.path().join("Main.qml.studio-tmp")).unwrap();
        store.write("Main.qml", "mine").unwrap();
        assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "theirs");
        assert_eq!(store.read("Main.qml").unwrap(), "mine");
    }

    #[test]
    fn a_document_is_a_qml_or_theme_file_outside_the_skipped_directories() {
        for good in [
            "Main.qml",
            "a/b/Card.qml",
            ".hidden.qml",
            "theme.json",
            "app/theme.json",
        ] {
            assert!(document_path(good), "{good}");
        }
        for bad in [
            "",
            ".env",
            ".git/config",
            "README.md",
            "other.json",
            "theme.json.bak",
            ".git/theme.json",
            ".git/Hook.qml",
            "a/.cache/B.qml",
            "node_modules/x/Y.qml",
            "target/G.qml",
            "build/G.qml",
            "a//B.qml",
            "./B.qml",
            "a/./B.qml",
            "a/../B.qml",
        ] {
            assert!(!document_path(bad), "{bad}");
        }
    }

    #[test]
    fn files_are_listed_relative_and_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        store.write("b/Two.qml", "Item {}").unwrap();
        store.write("One.qml", "Item {}").unwrap();
        store.write(".studio/Hidden.qml", "Item {}").unwrap();
        assert_eq!(store.list(".qml"), ["One.qml", "b/Two.qml"]);
    }
}
