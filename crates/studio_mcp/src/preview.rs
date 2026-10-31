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
//! [`RELOADS_PER_SECOND`] times a second; the writes in between are shown by
//! the preview's own file watcher a moment later, and whatever reads the
//! preview first asks for the reload they missed ([`Previews::catch_up`]).

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
    /// When the last reloads after writes were asked for, oldest first,
    /// within the last second.
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

pub(crate) type Shared = Arc<Mutex<Previews>>;

pub(crate) fn lock(p: &Mutex<Previews>) -> MutexGuard<'_, Previews> {
    p.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Previews {
    /// After a file was written: the open preview shows it now, or, in a
    /// burst, once its watcher sees the file or somebody reads it.
    pub fn wrote(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        let now = Instant::now();
        while open
            .reloads
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(1))
        {
            open.reloads.pop_front();
        }
        if open.reloads.len() >= RELOADS_PER_SECOND {
            open.behind = true;
            return;
        }
        open.reloads.push_back(now);
        self.reload();
    }

    /// Before the preview is read: the reload a burst of writes left out.
    pub fn catch_up(&mut self) {
        if self
            .open
            .as_mut()
            .is_some_and(|o| std::mem::take(&mut o.behind))
        {
            self.reload();
        }
    }

    fn reload(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        open.behind = false;
        let started = Instant::now();
        let event = match open.host.reload() {
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
