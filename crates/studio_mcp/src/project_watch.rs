//! Hears of the files other programs change in the project (DOGFOOD D16).
//! The kernel says when, so nothing polls, and an idle project costs a thread
//! blocked on a channel.

use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        mpsc::{Receiver, Sender, channel},
    },
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use studio_core::{document_path, listed};

/// What a document says, as far as the watch knows: a hash, `None` when gone.
type Said = Option<u64>;

#[derive(Default)]
struct Document {
    /// `None` until the disk first tells of it.
    last: Option<Said>,
    /// What the session wrote and the disk has not yet told of, oldest first.
    /// Each goes when told of, so a later save by another program that says
    /// what the session once said is still news. A write that failed stays
    /// until the bound pushes it out.
    own: VecDeque<Said>,
}

const OWN_WRITES_AHEAD: usize = 64;

type Known = Arc<Mutex<HashMap<String, Document>>>;

enum Message {
    Event(notify::Result<notify::Event>),
    Stop,
}

/// Stops following when dropped.
pub(crate) struct ProjectWatch {
    stop: Sender<Message>,
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
    pub(crate) fn start(root: PathBuf, told: Box<dyn Fn(News) + Send>) -> Option<Self> {
        let (stop, messages) = channel();
        let events = stop.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = events.send(Message::Event(event));
        })
        .ok()?;
        watch_tree(&mut watcher, &root, &root);
        let known = Known::default();
        let thread_known = known.clone();
        std::thread::Builder::new()
            .name("studio-project-watch".to_owned())
            .spawn(move || follow(watcher, &root, &thread_known, &messages, &*told))
            .ok()?;
        Some(Self { stop, known })
    }

    /// What the session calls before it writes (`Some`) or removes (`None`)
    /// a file: called after, the disk could tell of it first.
    pub(crate) fn own_writes(&self) -> impl Fn(&str, Option<&str>) + Send + Sync + 'static {
        let known = self.known.clone();
        move |path, text| {
            if document_path(path) {
                let mut known = known.lock().unwrap_or_else(PoisonError::into_inner);
                let own = &mut known.entry(path.to_owned()).or_default().own;
                if own.len() == OWN_WRITES_AHEAD {
                    own.pop_front();
                }
                own.push_back(text.map(|text| hashed(text.as_bytes())));
            }
        }
    }
}

/// What an event says, the most it says in a burst last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum News {
    /// A file that is no document, for who lists them.
    Other,
    /// A document may have changed: what the preview shows is behind.
    Document,
}

fn hashed(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn said(path: &Path) -> Said {
    Some(hashed(&std::fs::read(path).ok()?))
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
    told: &dyn Fn(News),
) {
    while let Ok(first) = messages.recv() {
        // A checkout comes as a burst: one telling reads all of it.
        let mut news = None;
        for message in std::iter::once(first).chain(messages.try_iter()) {
            match message {
                Message::Stop => return,
                Message::Event(Ok(event)) if !matches!(event.kind, EventKind::Access(_)) => {
                    for path in &event.paths {
                        news = news.max(news_of(&mut watcher, path, root, known));
                    }
                }
                Message::Event(_) => {}
            }
        }
        if let Some(news) = news {
            told(news);
        }
    }
}

fn news_of(
    watcher: &mut RecommendedWatcher,
    path: &Path,
    root: &Path,
    known: &Known,
) -> Option<News> {
    let rel = relative(path, root)?;
    let is_dir = path.symlink_metadata().is_ok_and(|m| m.is_dir());
    if is_dir {
        // A directory made with files already in it says nothing of them.
        if walked(path, root) {
            watch_tree(watcher, path, root);
            return Some(News::Document);
        }
        return None;
    }
    let name = rel.rsplit('/').next().unwrap_or_default();
    if !listed(&rel)
        || name.starts_with('.')
        || name.ends_with('~')
        || name.ends_with(".studio-tmp")
    {
        return None;
    }
    if !document_path(&rel) {
        // Not worth reading to tell. A directory gone may have taken
        // documents with it; what is gone says no more than its name.
        let gone = !name.contains('.') && path.symlink_metadata().is_err();
        return Some(if gone { News::Document } else { News::Other });
    }
    let now = said(path);
    let mut known = known.lock().unwrap_or_else(PoisonError::into_inner);
    let document = known.entry(rel).or_default();
    let was = document.last.replace(now);
    if let Some(at) = document.own.iter().position(|own| *own == now) {
        // The disk tells of a write once it has it, and of the ones before
        // it together with it.
        document.own.drain(..=at);
        return None;
    }
    (was != Some(now)).then_some(News::Document)
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

    /// As the store saves: all at once, so nothing reads half of it.
    fn saved(dir: &Path, name: &str, text: &str) {
        let temporary = dir.join(format!("{name}.studio-tmp"));
        std::fs::write(&temporary, text).unwrap();
        std::fs::rename(&temporary, dir.join(name)).unwrap();
    }

    fn watched(dir: &Path) -> (ProjectWatch, Arc<AtomicU64>) {
        let told = Arc::new(AtomicU64::new(0));
        let counter = told.clone();
        let watch = ProjectWatch::start(
            dir.canonicalize().unwrap(),
            Box::new(move |_| {
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
        for n in 0..50 {
            let text = format!("Item {{ width: {n} }}\n");
            own("Mine.qml", Some(&text));
            saved(dir.path(), "Mine.qml", &text);
        }
        std::fs::write(dir.path().join("Theirs.qml"), "Item {}\n").unwrap();
        waits_for("the later file", || told.load(Ordering::SeqCst) > 0);
        assert_eq!(told.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn another_programs_save_of_what_the_session_once_wrote_tells() {
        let dir = tempfile::tempdir().unwrap();
        let (watch, told) = watched(dir.path());
        let own = watch.own_writes();
        for text in ["Item { width: 1 }\n", "Item { width: 2 }\n"] {
            own("Mine.qml", Some(text));
            saved(dir.path(), "Mine.qml", text);
        }
        // Told of the session's last write, so the window has room for news.
        std::fs::write(dir.path().join("Marker.qml"), "Item {}\n").unwrap();
        waits_for("the marker", || told.load(Ordering::SeqCst) > 0);
        saved(dir.path(), "Mine.qml", "Item { width: 1 }\n");
        waits_for("the save", || told.load(Ordering::SeqCst) > 1);
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
    fn a_churning_log_is_no_news_of_a_document() {
        let dir = tempfile::tempdir().unwrap();
        let documents = Arc::new(AtomicU64::new(0));
        let others = Arc::new(AtomicU64::new(0));
        let (d, o) = (documents.clone(), others.clone());
        let _watch = ProjectWatch::start(
            dir.path().canonicalize().unwrap(),
            Box::new(move |news| {
                let count = if news == News::Document { &d } else { &o };
                count.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .expect("a watch");
        for n in 0..20 {
            std::fs::write(dir.path().join("app.log"), format!("{n}\n")).unwrap();
        }
        waits_for("the log", || others.load(Ordering::SeqCst) > 0);
        assert_eq!(documents.load(Ordering::SeqCst), 0);
        std::fs::write(dir.path().join("A.qml"), "Item {}\n").unwrap();
        waits_for("the document", || documents.load(Ordering::SeqCst) == 1);
    }

    #[test]
    fn a_file_saved_unchanged_tells_once() {
        let dir = tempfile::tempdir().unwrap();
        let (_watch, told) = watched(dir.path());
        saved(dir.path(), "A.qml", "Item {}\n");
        waits_for("the first save", || told.load(Ordering::SeqCst) == 1);
        saved(dir.path(), "A.qml", "Item {}\n");
        saved(dir.path(), "B.qml", "Item {}\n");
        waits_for("the later file", || told.load(Ordering::SeqCst) == 2);
        assert_eq!(told.load(Ordering::SeqCst), 2);
    }
}
