//! The preview the server shows: one at a time, of one document.
//!
//! The session tells it after every write (its own edits, undo, the code
//! view), and it asks its host to reload at once; what came of each reload
//! goes into an event list that `preview_events` reads with a cursor, so a
//! client learns of failures without anything polling.

use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use studio_core::{
    Store,
    preview::{Alias, PreviewEvent, PreviewHost, PreviewSpec, Reload, Size, Target, ms},
};

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
    /// After a file was written: the open preview shows it now.
    pub fn wrote(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        let started = Instant::now();
        let event = match open.host.reload() {
            Ok(Reload::Applied { generation }) => PreviewEvent::Reloaded {
                generation,
                ms: ms(started.elapsed()),
            },
            Ok(Reload::Failed { message }) => PreviewEvent::ReloadFailed { message },
            Ok(Reload::Unchanged) => return,
            Err(e) => {
                self.open = None;
                PreviewEvent::Ended {
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
            Err(e) => {
                self.open = None;
                self.events.push(PreviewEvent::Ended {
                    message: e.to_string(),
                });
            }
        }
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
