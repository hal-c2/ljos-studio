//! Where documents are read from and written to.
//!
//! The filesystem natively, browser storage on the web, memory in tests:
//! the session only ever sees this trait, so the same session runs on all
//! three.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("`{0}` is outside the project")]
    Outside(String),
    #[error("`{0}` does not exist")]
    Missing(String),
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
}

/// A project directory on disk.
#[derive(Debug)]
pub struct FsStore {
    root: PathBuf,
}

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
        Ok(self.root.join(relative))
    }
}

/// Directories a walk leaves out: build output and tools' own.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "node_modules" | "build")
}

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
        // reload) never reads half a file.
        let tmp = full.with_extension("qml.studio-tmp");
        std::fs::write(&tmp, text).map_err(io)?;
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

#[cfg(test)]
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
    fn files_are_listed_relative_and_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::new(dir.path()).unwrap();
        store.write("b/Two.qml", "Item {}").unwrap();
        store.write("One.qml", "Item {}").unwrap();
        store.write(".studio/Hidden.qml", "Item {}").unwrap();
        assert_eq!(store.list(".qml"), ["One.qml", "b/Two.qml"]);
    }
}
