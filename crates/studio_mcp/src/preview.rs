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
//! [`RELOADS_PER_SECOND`] times a second, whoever asks; the writes in between
//! are shown by the preview's own file watcher a moment later, and the reload
//! they missed is asked for once the budget allows ([`Previews::catch_up`]).
//! A reader that must see the last write waits for that ([`caught_up`]).

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use studio_core::{
    Store,
    preview::{
        Alias, PreviewError, PreviewEvent, PreviewHost, PreviewSpec, Reload, Size, Target, ms,
    },
};

/// Reloads asked for after writes in any second: well within the 20 calls
/// a second an inspector answers, leaving the rest to pictures and reads.
pub(crate) const RELOADS_PER_SECOND: usize = 8;

/// Makes a host for the project in a directory.
pub type HostFactory = Arc<dyn Fn(&std::path::Path) -> Box<dyn PreviewHost> + Send + Sync>;

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
    /// When the last reloads were answered, oldest first, within the last
    /// second.
    pub reloads: VecDeque<Instant>,
    /// A write was not reloaded: the next reading asks for it.
    pub behind: bool,
}

#[derive(Default)]
pub(crate) struct Previews {
    pub open: Option<Open>,
    pub events: Vec<PreviewEvent>,
    /// The host's log mark already read into `events`.
    pub log_mark: u64,
}

impl Open {
    /// When the budget next allows a reload; `None` when it does now.
    fn spent_until(&mut self) -> Option<Instant> {
        let now = Instant::now();
        while self
            .reloads
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(1))
        {
            self.reloads.pop_front();
        }
        if self.reloads.len() < RELOADS_PER_SECOND {
            return None;
        }
        self.reloads.front().map(|at| *at + Duration::from_secs(1))
    }
}

pub(crate) type Shared = Arc<Mutex<Previews>>;

pub(crate) fn lock(p: &Mutex<Previews>) -> MutexGuard<'_, Previews> {
    p.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The previews with every write shown, for a reader that names what it
/// reads by the session's text: waits for the budget if a burst spent it.
/// Called with the session locked, so no later write can put it behind again.
pub(crate) fn caught_up(p: &Mutex<Previews>) -> MutexGuard<'_, Previews> {
    loop {
        let mut previews = lock(p);
        let Some(until) = previews.catch_up() else {
            return previews;
        };
        // Not holding the previews: the timer's catch-up may get there first.
        drop(previews);
        std::thread::sleep(until.saturating_duration_since(Instant::now()));
    }
}

impl Previews {
    /// After a file was written: the open preview shows it now, or, in a
    /// burst, once its watcher sees the file or somebody reads it. In a
    /// burst, the first write past the budget answers when the budget next
    /// allows a reload, for the caller to catch up then.
    pub fn wrote(&mut self) -> Option<Instant> {
        let open = self.open.as_mut()?;
        if let Some(until) = open.spent_until() {
            let first = !std::mem::replace(&mut open.behind, true);
            return first.then_some(until);
        }
        self.reload();
        None
    }

    /// The reload a burst of writes left out, if the budget allows it now.
    /// If not, answers when it will; the preview stays behind and the timer
    /// [`wrote`](Self::wrote) asked for is still to come.
    pub fn catch_up(&mut self) -> Option<Instant> {
        let open = self.open.as_mut()?;
        if !open.behind {
            return None;
        }
        if let Some(until) = open.spent_until() {
            return Some(until);
        }
        self.reload();
        None
    }

    fn reload(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        open.behind = false;
        let started = Instant::now();
        let reload = open.host.reload();
        // Stamped once answered: however long the call took, the reload the
        // budget allows a second after this reaches the inspector more than
        // a second after this one did.
        open.reloads.push_back(Instant::now());
        let event = match reload {
            Ok(Reload::Applied { generation }) => PreviewEvent::Reloaded {
                generation,
                ms: ms(started.elapsed()),
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

    pub fn record(&mut self, reload: Reload, started: Instant) {
        match reload {
            Reload::Applied { generation } => self.events.push(PreviewEvent::Reloaded {
                generation,
                ms: ms(started.elapsed()),
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

pub(crate) fn spec(doc: &str, size: Size, aliases: &[Alias]) -> PreviewSpec {
    PreviewSpec {
        entry: doc.to_owned(),
        size,
        aliases: aliases.to_vec(),
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

    fn previews(reloads: VecDeque<Instant>, behind: bool) -> (Previews, Arc<Mutex<usize>>) {
        let count = Arc::new(Mutex::new(0));
        let previews = Previews {
            open: Some(Open {
                host: Box::new(Counting(count.clone())),
                doc: "Main.qml".to_owned(),
                target: None,
                size: Target::Desktop.size(),
                state: State::Live,
                mocked: Vec::new(),
                reloads,
                behind,
            }),
            ..Previews::default()
        };
        (previews, count)
    }

    #[test]
    fn a_catch_up_with_the_budget_spent_stays_behind_and_says_until_when() {
        // In the future, so no pause of this test can let them expire.
        let at = Instant::now() + Duration::from_secs(60);
        let (mut p, count) = previews(vec![at; RELOADS_PER_SECOND].into(), true);
        assert_eq!(p.catch_up(), Some(at + Duration::from_secs(1)));
        assert_eq!(*count.lock().unwrap(), 0);
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
