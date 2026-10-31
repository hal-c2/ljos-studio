//! Studio's state directory: the token files and preview logs of one run.
//!
//! Without `XDG_RUNTIME_DIR` it is in persistent storage (`XDG_STATE_HOME`
//! or `~/.local/state`), which nothing clears at logout. So a run removes
//! its own when it ends, and one a run left behind when it was killed is
//! removed by the next start: each run holds a lock on a file in its
//! directory while it lives, and a directory whose lock can be taken has no
//! run left. A lock, not the pid in the name, says so, as a pid is reused.
//!
//! The lock file is made and locked under another name and only then
//! renamed into place, so another start's sweep never sees it unlocked:
//! between making a file and locking it, that sweep could take the lock and
//! remove the directory of a run that is starting.

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Context;
use studio_preview::private_dir;

const PREFIX: &str = "ljos-studio-";
const LOCK: &str = "running";
/// The lock file before it is locked: no sweep looks at it.
const MAKING: &str = "running.new";

/// This run's state directory, removed when dropped.
pub struct StateDir {
    path: PathBuf,
    lock: Option<File>,
}

impl StateDir {
    /// Removes what ended runs left below `base`, then makes this run's
    /// directory there, this user's alone, and holds its lock.
    pub fn new(base: &Path) -> anyhow::Result<Self> {
        Self::make(base, || {})
    }

    /// [`new`](Self::new); `unlocked` runs when the lock file is made and
    /// not yet locked, where a test puts another start's sweep.
    fn make(base: &Path, unlocked: impl FnOnce()) -> anyhow::Result<Self> {
        sweep(base);
        let path = base.join(format!("{PREFIX}{}", std::process::id()));
        private_dir(&path)?;
        let making = path.join(MAKING);
        let lock = File::create(&making).with_context(|| format!("making {}", making.display()))?;
        unlocked();
        lock.try_lock()
            .with_context(|| format!("locking {}", making.display()))?;
        let lock_path = path.join(LOCK);
        std::fs::rename(&making, &lock_path)
            .with_context(|| format!("naming {}", lock_path.display()))?;
        Ok(Self {
            path,
            lock: Some(lock),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StateDir {
    fn drop(&mut self) {
        // Closed first: an open file cannot be removed everywhere.
        drop(self.lock.take());
        if let Err(e) = std::fs::remove_dir_all(&self.path) {
            eprintln!("studio: {} is left behind: {e}", self.path.display());
        }
    }
}

/// Removes every `ljos-studio-*` directory below `base` whose run has
/// ended. One with no lock file is left: its run may be making it now. So
/// is one a run was killed in while it made its lock, which only a person
/// can tell from one being made.
fn sweep(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(PREFIX) {
            continue;
        }
        // Not followed: a link here is nobody's run.
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let dir = entry.path();
        let Ok(lock) = File::open(dir.join(LOCK)) else {
            continue;
        };
        // Taken: its run has ended. Refused: it lives, or the lock cannot
        // be judged, and a directory is never removed on a guess.
        if lock.try_lock().is_ok() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(base: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(base)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_run_removes_its_directory_when_it_ends() {
        let base = tempfile::tempdir().unwrap();
        let state = StateDir::new(base.path()).unwrap();
        std::fs::write(state.path().join("preview.log"), "a log").unwrap();
        assert!(state.path().is_dir());
        drop(state);
        assert_eq!(names(base.path()), Vec::<String>::new());
    }

    #[test]
    fn a_killed_runs_directory_is_removed_by_the_next_start() {
        let base = tempfile::tempdir().unwrap();
        // What a killed run leaves: its lock file, no longer locked.
        let killed = base.path().join("ljos-studio-1");
        std::fs::create_dir(&killed).unwrap();
        std::fs::write(killed.join(LOCK), "").unwrap();
        std::fs::write(killed.join("preview.log"), "a log").unwrap();
        let state = StateDir::new(base.path()).unwrap();
        assert_eq!(
            names(base.path()),
            vec![format!("ljos-studio-{}", std::process::id())]
        );
        drop(state);
    }

    #[test]
    fn another_starts_sweep_leaves_a_run_that_is_making_its_lock() {
        let base = tempfile::tempdir().unwrap();
        let state = StateDir::make(base.path(), || sweep(base.path())).unwrap();
        assert!(state.path().join(LOCK).is_file());
        // Held: a sweep after the start leaves it too.
        sweep(base.path());
        assert_eq!(
            names(base.path()),
            vec![format!("ljos-studio-{}", std::process::id())]
        );
        drop(state);
    }

    #[test]
    fn a_running_runs_directory_and_anything_else_are_left() {
        let base = tempfile::tempdir().unwrap();
        let running = base.path().join("ljos-studio-2");
        std::fs::create_dir(&running).unwrap();
        let held = File::create(running.join(LOCK)).unwrap();
        held.try_lock().unwrap();
        // Being made: no lock file yet.
        std::fs::create_dir(base.path().join("ljos-studio-3")).unwrap();
        std::fs::create_dir(base.path().join("other")).unwrap();
        #[cfg(unix)]
        {
            // A link is nobody's run, whatever it points at.
            let ended = base.path().join("ended");
            std::fs::create_dir(&ended).unwrap();
            std::fs::write(ended.join(LOCK), "").unwrap();
            std::os::unix::fs::symlink(&ended, base.path().join("ljos-studio-4")).unwrap();
        }
        let state = StateDir::new(base.path()).unwrap();
        drop(state);
        let mut left = vec!["ljos-studio-2", "ljos-studio-3", "other"];
        if cfg!(unix) {
            left = vec![
                "ended",
                "ljos-studio-2",
                "ljos-studio-3",
                "ljos-studio-4",
                "other",
            ];
        }
        assert_eq!(names(base.path()), left);
    }
}
