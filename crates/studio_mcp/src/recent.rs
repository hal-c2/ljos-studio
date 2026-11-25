//! The folders a person opened last, newest first, kept in a file so the
//! next start can offer them again.

use std::path::{Path, PathBuf};

/// As many as a menu shows.
const MOST: usize = 10;

#[derive(Debug, Default)]
pub(crate) struct Recent {
    /// Where they are kept; none keeps them only while Studio runs.
    file: Option<PathBuf>,
    pub(crate) folders: Vec<PathBuf>,
}

impl Recent {
    /// What `file` says; a missing or broken file is no folders yet.
    pub(crate) fn load(file: PathBuf) -> Self {
        let folders = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<PathBuf>>(&text).ok())
            .unwrap_or_default();
        Self {
            file: Some(file),
            folders,
        }
    }

    /// `folder` first. A file that cannot be written loses only the
    /// offer next time, so it does not refuse the folder that opened.
    pub(crate) fn remember(&mut self, folder: &Path) {
        self.folders.retain(|f| f != folder);
        self.folders.insert(0, folder.to_owned());
        self.folders.truncate(MOST);
        let Some(file) = &self.file else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&self.folders) {
            let _ = std::fs::write(file, text);
        }
    }

    pub(crate) fn listed(&self) -> Vec<String> {
        self.folders
            .iter()
            .filter_map(|f| f.to_str().map(str::to_owned))
            .collect()
    }
}
