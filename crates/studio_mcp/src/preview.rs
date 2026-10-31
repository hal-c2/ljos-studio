//! The preview the server shows: one at a time, of one document.
//!
//! The session tells it after every write (its own edits, undo, the code
//! view), and it asks its host to reload at once; what came of each reload
//! goes into an event list that `preview_events` reads with a cursor, so a
//! client learns of failures without anything polling.
//!
//! A preview's inspector answers a bounded number of calls a second (Ljós
//! ARCHITECTURE.md 19.7), and one that asks too often waits a second for its
//! answer. So a burst of writes (typing in the code view) reloads at most
//! [`reloads_per_second`] times a second, whoever asks; the writes in between
//! are shown by the preview's own file watcher a moment later, and the reload
//! they missed is asked for once the budget allows ([`Previews::catch_up`]).
//! A reader that must see the last write waits for that ([`caught_up`]).

use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use crate::timer::Timer;
use studio_core::{
    Store,
    preview::{
        Alias, PreviewError, PreviewEvent, PreviewHost, PreviewSpec, Reload, Size, Target, ms,
    },
};

/// Reloads asked for after writes in any second: two fifths of the calls
/// the preview answers, leaving the rest to pictures and reads.
pub(crate) fn reloads_per_second(calls_per_second: u32) -> usize {
    (calls_per_second as usize * 2 / 5).max(1)
}

/// Makes a host for the project in a directory.
pub type HostFactory = Arc<dyn Fn(&std::path::Path) -> Box<dyn PreviewHost> + Send + Sync>;

/// Makes a host for a project that is not in a directory (browser storage):
/// the host gets the files from wherever the project's store is.
pub type StoreHostFactory = Arc<dyn Fn() -> Box<dyn PreviewHost> + Send + Sync>;

/// Where a server's previews come from.
#[derive(Clone)]
pub(crate) enum Hosts {
    /// A host that runs the project from its directory (a child process).
    InDir(HostFactory),
    /// A host that needs no directory.
    Anywhere(StoreHostFactory),
}

impl Hosts {
    /// A host for the project at `dir`, when this kind of host can run it.
    pub fn make(&self, dir: Option<&std::path::Path>) -> Option<Box<dyn PreviewHost>> {
        match self {
            Hosts::InDir(f) => dir.map(|d| f(d)),
            Hosts::Anywhere(f) => Some(f()),
        }
    }

    pub fn runs(&self, dir: Option<&std::path::Path>) -> bool {
        matches!(self, Hosts::Anywhere(_)) || dir.is_some()
    }
}

/// Whether the blocks are the app's own or its mock modules'.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Live,
    Mocked,
}

pub(crate) struct Open {
    pub host: Box<dyn PreviewHost>,
    pub doc: String,
    pub target: Option<Target>,
    pub size: Size,
    pub state: State,
    /// The aliases in force: empty when live.
    pub mocked: Vec<Alias>,
    /// When the last reloads were answered (the timer's time), oldest
    /// first, within the last second.
    pub reloads: VecDeque<Duration>,
    /// A write was not reloaded: the next reading asks for it.
    pub behind: bool,
    /// Clicks given to the running app: they change what it shows without a
    /// reload, so a window has to know to look again.
    pub clicks: u64,
    /// The object and the state `preview_state` shows: a reload starts the
    /// app again in its base state, so it is shown again.
    pub shown: Option<(String, String)>,
}

pub(crate) struct Previews {
    /// The ids of the objects `node_arrange` `hide` hides, which outlast a
    /// reload and a preview. Written to the running app, never to the file.
    pub(crate) hidden: BTreeSet<String>,
    pub open: Option<Open>,
    pub events: Vec<PreviewEvent>,
    /// The host's log mark already read into `events`.
    pub log_mark: u64,
    pub timer: Arc<dyn Timer>,
}

impl Open {
    /// How long from `now` (the timer's time) until the budget next allows
    /// a reload; `None` when it does now.
    fn spent_until(&mut self, now: Duration) -> Option<Duration> {
        while self
            .reloads
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= Duration::from_secs(1))
        {
            self.reloads.pop_front();
        }
        if self.reloads.len() < reloads_per_second(self.host.calls_per_second()) {
            return None;
        }
        self.reloads
            .front()
            .map(|at| (*at + Duration::from_secs(1)).saturating_sub(now))
    }
}

pub(crate) type Shared = Arc<Mutex<Previews>>;

pub(crate) fn lock(p: &Mutex<Previews>) -> MutexGuard<'_, Previews> {
    p.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The previews with every write shown, for a reader that names what it
/// reads by the session's text: waits for the budget if a burst spent it.
/// Called with the session locked, so no later write can put it behind again.
///
/// A timer that cannot wait (a page's one thread) reloads past the budget
/// instead: the budget is only Studio's pace there, and a reader that named
/// the last write must not be shown the one before it.
pub(crate) fn caught_up(p: &Mutex<Previews>) -> MutexGuard<'_, Previews> {
    loop {
        let mut previews = lock(p);
        let Some(delay) = previews.catch_up() else {
            return previews;
        };
        let timer = Arc::clone(&previews.timer);
        // Not holding the previews: the timer's catch-up may get there first.
        drop(previews);
        if !timer.wait(delay) {
            let mut previews = lock(p);
            if previews.open.as_ref().is_some_and(|open| open.behind) {
                previews.reload();
            }
            return previews;
        }
    }
}

impl Previews {
    pub fn new(timer: Arc<dyn Timer>) -> Self {
        Self {
            open: None,
            events: Vec::new(),
            log_mark: 0,
            timer,
            hidden: BTreeSet::new(),
        }
    }

    /// Whether the object with `id` is hidden.
    pub(crate) fn is_hidden(&self, id: &str) -> bool {
        self.hidden.contains(id)
    }

    /// Hides an object in the preview, now and after each reload.
    pub(crate) fn hide(&mut self, id: String) {
        self.hidden.insert(id);
        self.write_hidden();
    }

    /// Shows it again with `visible`, what its file says. The file has not
    /// changed, so a reload would change nothing: a `visible` the file binds
    /// to an expression holds this value until the next reload.
    pub(crate) fn show(&mut self, id: &str, visible: bool) {
        if !self.hidden.remove(id) {
            return;
        }
        if let Some(open) = &mut self.open {
            // An object the text no longer has has nothing to show.
            let _ = open.host.write(id, "visible", visible.into());
            open.clicks += 1;
        }
    }

    /// What `node_arrange` `hide` holds, on what a reload or a new app put back.
    pub(crate) fn write_hidden(&mut self) {
        let Some(open) = self.open.as_mut().filter(|_| !self.hidden.is_empty()) else {
            return;
        };
        for id in &self.hidden {
            // An object the text no longer has has nothing to hide.
            let _ = open.host.write(id, "visible", false.into());
        }
        open.clicks += 1;
    }

    /// After a file was written: the open preview shows it now, or, in a
    /// burst, once its watcher sees the file or somebody reads it. In a
    /// burst, the first write past the budget answers when the budget next
    /// allows a reload (how long from now), for the caller to catch up then.
    pub fn wrote(&mut self) -> Option<Duration> {
        let now = self.timer.now();
        let open = self.open.as_mut()?;
        if let Some(until) = open.spent_until(now) {
            let first = !std::mem::replace(&mut open.behind, true);
            return first.then_some(until);
        }
        self.reload();
        None
    }

    /// The reload a burst of writes left out, if the budget allows it now.
    /// If not, answers how long from now it will; the preview stays behind
    /// and the timer [`wrote`](Self::wrote) asked for is still to come.
    pub fn catch_up(&mut self) -> Option<Duration> {
        let now = self.timer.now();
        let open = self.open.as_mut()?;
        if !open.behind {
            return None;
        }
        if let Some(until) = open.spent_until(now) {
            return Some(until);
        }
        self.reload();
        None
    }

    fn reload(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        let started = self.timer.now();
        let reload = open.host.reload();
        // Only now: a host that panics has not shown the write, and the lock
        // outlives the panic for the next reader to catch up.
        open.behind = false;
        if let (Ok(Reload::Applied { .. }), Some((id, state))) = (&reload, &open.shown) {
            // Out and in again: the app comes back in the state it was in,
            // and setting that same state would not apply what the new text
            // adds to it. One the new text no longer has is the app's to
            // warn about.
            for value in ["", state.as_str()] {
                let _ = open.host.write(id, "state", value.into());
            }
            // Drawn after the reload's picture was asked for, as a click is.
            open.clicks += 1;
        }
        if matches!(reload, Ok(Reload::Applied { .. })) {
            for id in &self.hidden {
                let _ = open.host.write(id, "visible", false.into());
            }
        }

        // Stamped once answered: however long the call took, the reload the
        // budget allows a second after this reaches the inspector more than
        // a second after this one did.
        open.reloads.push_back(self.timer.now());
        let event = match reload {
            Ok(Reload::Applied { generation }) => PreviewEvent::Reloaded {
                generation,
                ms: ms(self.timer.now().saturating_sub(started)),
            },
            Ok(Reload::Failed { message }) => PreviewEvent::ReloadFailed { message },
            Ok(Reload::Unchanged) => return,
            Err(e @ (PreviewError::Ended(_) | PreviewError::NotOpen)) => {
                self.failed(e);
                return;
            }
            // The process is still there and may still be showing the last
            // good text: a refusal (too many calls a second) or a broken
            // connection is one reload missed, and the next reading asks
            // again.
            Err(e) => {
                open.behind = true;
                PreviewEvent::ReloadFailed {
                    message: e.to_string(),
                }
            }
        };
        self.events.push(event);
    }

    /// What a reload asked for at `started` (the timer's time) did.
    pub fn record(&mut self, reload: Reload, started: Duration) {
        match reload {
            Reload::Applied { generation } => self.events.push(PreviewEvent::Reloaded {
                generation,
                ms: ms(self.timer.now().saturating_sub(started)),
            }),
            Reload::Failed { message } => {
                self.events.push(PreviewEvent::ReloadFailed { message });
            }
            Reload::Unchanged => {}
        }
    }

    /// Reads the host's new log lines into the events.
    pub fn pull_logs(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        match open.host.logs(self.log_mark) {
            Ok((lines, next)) => {
                self.log_mark = next;
                self.events.extend(lines.into_iter().map(PreviewEvent::Log));
            }
            Err(e @ (PreviewError::Ended(_) | PreviewError::NotOpen)) => {
                self.failed(e);
            }
            Err(e) => {
                open.behind = true;
                self.events.push(PreviewEvent::ReloadFailed {
                    message: e.to_string(),
                });
            }
        }
    }

    /// What every host call's error goes through: one that says the preview
    /// is over ends it here too, or the next `preview_open` would reuse a
    /// process that is gone. Any other leaves it open.
    pub fn failed(&mut self, e: PreviewError) -> PreviewError {
        if matches!(e, PreviewError::Ended(_) | PreviewError::NotOpen) {
            self.open = None;
            self.events.push(PreviewEvent::Ended {
                message: e.to_string(),
            });
        }
        e
    }
}

/// The modules the project's `mocks/` directory stands in for: each
/// `qmldir` below it, by its `module` line.
pub(crate) fn mocks(store: &dyn Store) -> Vec<Alias> {
    let mut out: Vec<Alias> = store
        .list("qmldir")
        .into_iter()
        .filter(|path| path.starts_with("mocks/") && path.ends_with("/qmldir"))
        .filter_map(|path| {
            let text = store.read(&path).ok()?;
            let module = text.lines().find_map(|l| {
                let mut words = l.split_whitespace();
                (words.next() == Some("module"))
                    .then(|| words.next())
                    .flatten()
            })?;
            let dir = path.strip_suffix("/qmldir")?;
            Some(Alias {
                module: module.to_owned(),
                dir: dir.to_owned(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.module.cmp(&b.module));
    out
}

pub(crate) fn spec(store: &dyn Store, doc: &str, size: Size, aliases: &[Alias]) -> PreviewSpec {
    let mut types = crate::blocks::dumps(store);
    types.sort();
    PreviewSpec {
        entry: doc.to_owned(),
        size,
        aliases: aliases.to_vec(),
        types,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value};
    use studio_core::preview::{Declared, LogLine, Picture, PreviewError};

    use super::*;

    /// Answers reloads and counts them; nothing else is asked of it here.
    struct Counting(Arc<Mutex<usize>>);

    impl PreviewHost for Counting {
        fn open(&mut self, _: &PreviewSpec) -> Result<(), PreviewError> {
            unreachable!()
        }
        fn reload(&mut self) -> Result<Reload, PreviewError> {
            *self.0.lock().unwrap() += 1;
            Ok(Reload::Unchanged)
        }
        fn alias(&mut self, _: &str, _: Option<&str>) -> Result<Reload, PreviewError> {
            unreachable!()
        }
        fn resize(&mut self, _: Size) -> Result<(), PreviewError> {
            unreachable!()
        }
        fn screenshot(&mut self, _: u32) -> Result<Picture, PreviewError> {
            unreachable!()
        }
        fn outline(&mut self) -> Result<String, PreviewError> {
            unreachable!()
        }
        fn read(&mut self, _: &str, _: &[String]) -> Result<Map<String, Value>, PreviewError> {
            unreachable!()
        }
        fn write(&mut self, _: &str, _: &str, _: Value) -> Result<(), PreviewError> {
            unreachable!()
        }
        fn logs(&mut self, _: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
            unreachable!()
        }
        fn pick(&mut self, _: f64, _: f64) -> Result<Option<Declared>, PreviewError> {
            unreachable!()
        }
        fn alive(&mut self) -> Result<(), PreviewError> {
            Ok(())
        }
        fn close(&mut self) {}
    }

    /// Time that stands still at ten seconds, and never waits.
    struct Still;

    impl Timer for Still {
        fn now(&self) -> Duration {
            Duration::from_secs(10)
        }
        fn wall(&self) -> Duration {
            Duration::ZERO
        }
        fn after(&self, _: Duration, _: Box<dyn FnOnce() + Send>) {}
    }

    fn previews(reloads: VecDeque<Duration>, behind: bool) -> (Previews, Arc<Mutex<usize>>) {
        let count = Arc::new(Mutex::new(0));
        let previews = Previews {
            hidden: BTreeSet::new(),
            open: Some(Open {
                host: Box::new(Counting(count.clone())),
                doc: "Main.qml".to_owned(),
                target: None,
                size: Target::Desktop.size(),
                state: State::Live,
                mocked: Vec::new(),
                reloads,
                behind,
                clicks: 0,
                shown: None,
            }),
            ..Previews::new(Arc::new(Still))
        };
        (previews, count)
    }

    #[test]
    fn a_catch_up_with_the_budget_spent_stays_behind_and_says_until_when() {
        // A quarter second ago by a timer that stands still, so no pause of
        // this test can let them expire.
        let at = Still.now() - Duration::from_millis(250);
        let (mut p, count) = previews(
            vec![at; reloads_per_second(Counting(Default::default()).calls_per_second())].into(),
            true,
        );
        assert_eq!(p.catch_up(), Some(Duration::from_millis(750)));
        assert_eq!(*count.lock().unwrap(), 0);
        assert!(p.open.as_ref().unwrap().behind);
    }

    #[test]
    fn a_host_that_panics_leaves_the_preview_behind() {
        let (mut p, count) = previews(VecDeque::new(), true);
        // A poisoned count makes the stub host's next reload panic.
        let held = Arc::clone(&count);
        std::thread::spawn(move || {
            let _held = held.lock().unwrap();
            panic!("poisoning the count");
        })
        .join()
        .unwrap_err();
        let reload = std::panic::AssertUnwindSafe(|| p.catch_up());
        assert!(std::panic::catch_unwind(reload).is_err());
        assert!(p.open.as_ref().unwrap().behind);
    }

    #[test]
    fn a_catch_up_counts_against_the_budget() {
        let (mut p, count) = previews(VecDeque::new(), true);
        assert_eq!(p.catch_up(), None);
        assert_eq!(*count.lock().unwrap(), 1);
        let open = p.open.as_ref().unwrap();
        assert!(!open.behind);
        assert_eq!(open.reloads.len(), 1);
    }
}
