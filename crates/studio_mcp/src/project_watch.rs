//! Hears of the files other programs change in the project (DOGFOOD D16).
//! The kernel says when, so nothing polls, and an idle project costs a thread
//! blocked on a channel.

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        mpsc::{Receiver, Sender, channel},
    },
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use studio_core::{document_path, listed};

/// What a document last said, as far as the watch knows: `None` when gone.
type Known = Arc<Mutex<HashMap<String, Option<u64>>>>;

enum Message {
    Event(notify::Result<notify::Event>),
    Stop,
}

/// Stops following when dropped.
pub(crate) struct ProjectWatch {
    stop: Sender<Message>,
    root: PathBuf,
    known: Known,
}

impl Drop for ProjectWatch {
    fn drop(&mut self) {
        let _ = self.stop.send(Message::Stop);
    }
}

impl ProjectWatch {
    /// `None` when the system gives no watch (out of inotify watches, say):
    /// a window then hears of such files at the next tool call, as it did.
    pub(crate) fn start(root: PathBuf, told: Box<dyn Fn() + Send>) -> Option<Self> {
        let (stop, messages) = channel();
        let events = stop.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = events.send(Message::Event(event));
        })
        .ok()?;
        watch_tree(&mut watcher, &root, &root);
        let known = Known::default();
        let (thread_root, thread_known) = (root.clone(), known.clone());
        std::thread::Builder::new()
            .name("studio-project-watch".to_owned())
            .spawn(move || follow(watcher, &thread_root, &thread_known, &messages, &*told))
            .ok()?;
        Some(Self { stop, root, known })
    }

    /// What the session calls with each path it writes or removes.
    pub(crate) fn own_writes(&self) -> impl Fn(&str) + Send + 'static {
        let (root, known) = (self.root.clone(), self.known.clone());
        move |path| {
            if document_path(path) {
                let said = said(&root.join(path));
                known
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(path.to_owned(), said);
            }
        }
    }
}

fn said(path: &Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

/// Each directory by itself, not the root recursively: the directories a
/// walk leaves out (`target`, `node_modules`) can hold more than the system
/// has watches for, and their churn is nobody's document. A link is not
/// followed, so the watch cannot leave the project.
fn watch_tree(watcher: &mut RecommendedWatcher, dir: &Path, root: &Path) {
    // Files made before the watch took are caught by the walk of whoever made
    // the directory; one that cannot be watched stays quiet.
    let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        if is_dir && walked(&entry.path(), root) {
            watch_tree(watcher, &entry.path(), root);
        }
    }
}

fn relative(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()?
        .to_str()
        .map(|rel| rel.replace('\\', "/"))
}

fn walked(dir: &Path, root: &Path) -> bool {
    relative(dir, root).is_some_and(|rel| listed(&format!("{rel}/file")))
}

fn follow(
    mut watcher: RecommendedWatcher,
    root: &Path,
    known: &Known,
    messages: &Receiver<Message>,
    told: &dyn Fn(),
) {
    while let Ok(first) = messages.recv() {
        // A checkout comes as a burst: one telling reads all of it.
        let mut news = false;
        for message in std::iter::once(first).chain(messages.try_iter()) {
            match message {
                Message::Stop => return,
                Message::Event(Ok(event)) if !matches!(event.kind, EventKind::Access(_)) => {
                    for path in &event.paths {
                        news |= is_news(&mut watcher, path, root, known);
                    }
                }
                Message::Event(_) => {}
            }
        }
        if news {
            told();
        }
    }
}

fn is_news(watcher: &mut RecommendedWatcher, path: &Path, root: &Path, known: &Known) -> bool {
    let Some(rel) = relative(path, root) else {
        return false;
    };
    let is_dir = path.symlink_metadata().is_ok_and(|m| m.is_dir());
    if is_dir {
        // A directory made with files already in it says nothing of them.
        if walked(path, root) {
            watch_tree(watcher, path, root);
            return true;
        }
        return false;
    }
    let name = rel.rsplit('/').next().unwrap_or_default();
    if !listed(&rel)
        || name.starts_with('.')
        || name.ends_with('~')
        || name.ends_with(".studio-tmp")
    {
        return false;
    }
    if !document_path(&rel) {
        // An asset or a directory gone: not worth reading to tell.
        return true;
    }
    let now = said(path);
    let mut known = known.lock().unwrap_or_else(PoisonError::into_inner);
    known.insert(rel, now) != Some(now)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    use super::*;

    fn waits_for(what: &str, seen: impl Fn() -> bool) {
        let started = Instant::now();
        while !seen() {
            assert!(started.elapsed() < Duration::from_secs(5), "never: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn watched(dir: &Path) -> (ProjectWatch, Arc<AtomicU64>) {
        let told = Arc::new(AtomicU64::new(0));
        let counter = told.clone();
        let watch = ProjectWatch::start(
            dir.canonicalize().unwrap(),
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .expect("a watch");
        (watch, told)
    }

    /// Events keep their order, so the telling for a later file proves the
    /// earlier ones were heard and judged.
    #[test]
    fn the_sessions_own_write_tells_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let (watch, told) = watched(dir.path());
        let own = watch.own_writes();
        std::fs::write(dir.path().join("Mine.qml"), "Item {}\n").unwrap();
        own("Mine.qml");
        std::fs::write(dir.path().join("Theirs.qml"), "Item {}\n").unwrap();
        waits_for("the later file", || told.load(Ordering::SeqCst) > 0);
        assert_eq!(told.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn what_a_walk_leaves_out_tells_nobody() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        let (_watch, told) = watched(dir.path());
        std::fs::write(dir.path().join("target/Out.qml"), "Item {}\n").unwrap();
        std::fs::write(dir.path().join(".Main.qml.swp"), "x").unwrap();
        std::fs::write(dir.path().join("Main.qml.studio-tmp"), "x").unwrap();
        std::fs::write(dir.path().join("Theirs.qml"), "Item {}\n").unwrap();
        waits_for("the later file", || told.load(Ordering::SeqCst) > 0);
        assert_eq!(told.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_file_saved_unchanged_tells_once() {
        let dir = tempfile::tempdir().unwrap();
        let (_watch, told) = watched(dir.path());
        std::fs::write(dir.path().join("A.qml"), "Item {}\n").unwrap();
        waits_for("the first save", || told.load(Ordering::SeqCst) == 1);
        std::fs::write(dir.path().join("A.qml"), "Item {}\n").unwrap();
        std::fs::write(dir.path().join("B.qml"), "Item {}\n").unwrap();
        waits_for("the later file", || told.load(Ordering::SeqCst) == 2);
        assert_eq!(told.load(Ordering::SeqCst), 2);
    }
}
