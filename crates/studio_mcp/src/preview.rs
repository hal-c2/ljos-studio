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
//! `reloads_per_second` times a second, whoever asks; the writes in between
//! are shown by the preview's own file watcher a moment later, and the reload
//! they missed is asked for once the budget allows (`Previews::catch_up`).
//! A reader that must see the last write waits for that (`caught_up`).

use std::{
    collections::{BTreeSet, VecDeque},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use crate::timer::Timer;
use studio_core::{
    Store,
    preview::{
        Alias, Extent, LogLine, PreviewError, PreviewEvent, PreviewHost, PreviewSpec, Reload, Size,
        Target, ms,
    },
};

/// Reloads asked for after writes in any second, with the read of their
/// logs: two fifths of the calls the preview answers, leaving the rest to
/// pictures and reads.
pub(crate) fn reloads_per_second(calls_per_second: u32) -> usize {
    // Each reload is two calls: the reload and the read of what it logged.
    (calls_per_second as usize * 2 / 5 / 2).max(1)
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

/// How tall a preview is: its screen's height, or as tall as what its
/// document lays out (never shorter than the screen).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Screen,
    Content,
}

pub(crate) struct Open {
    /// Which preview this is, of those `Previews` started: a call that lets
    /// go of the lock finds its own again, or knows it was replaced.
    pub started: u64,
    pub host: Box<dyn PreviewHost>,
    pub doc: String,
    pub target: Option<Target>,
    /// The size laid out for: `screen`, or taller when fitted to content.
    pub size: Size,
    /// The target's (or asked-for) size, before any fitting.
    pub screen: Size,
    pub fit: Fit,
    /// What the page measured at the height it was last grown to, fitted to
    /// content: how far it reached and what the host left out as the
    /// window's size. An item left out so can be what made the preview that
    /// tall, so an unchanged page can measure shorter than the preview; it
    /// measures all of this again.
    pub measured: Option<Extent>,
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
    /// The root's id and the state its breakpoint turns on at this width. A
    /// `when` a reload writes is not looked at until what it reads changes,
    /// and a window that stays one width never changes it, so it is shown as
    /// `shown` is; and `shown` wins over it.
    pub breakpoint: Option<(String, String)>,
    /// The file the preview runs in the document's place, when it needs one
    /// to show anything (`harness`). After `host`, so it goes after it.
    pub harness: Option<crate::harness::Standing>,
}

/// The most lines kept unread for the events, and the most problems of
/// what an app logs: as many lines as Ljós's own log keeps. A window reads
/// the log as the app writes it, and an app may warn for ever.
const LINES_KEPT: usize = 1024;

/// What a window shows of the previews that a call can change without
/// writing.
#[derive(PartialEq)]
pub(crate) struct Shown {
    problems: u64,
    open: Option<(String, Option<Target>, Size, State, u64)>,
}

pub(crate) struct Previews {
    /// The objects `node_arrange` `hide` hides, by document and handle, which
    /// outlast a reload and a preview: an id can pass to another object, a
    /// handle is never used again. Written to the running app, never to the
    /// file.
    pub(crate) hidden: BTreeSet<(String, String)>,
    /// The ids those objects have now, by document, which is what the running
    /// app knows them by. Only the session knows them, and a reload can come
    /// while it is busy or on a timer's thread, so `Studio::hidden_now` keeps
    /// them. An id means something in one document only, so only the
    /// document's own preview hears of them.
    pub(crate) hidden_ids: BTreeSet<(String, String)>,
    /// Emptied only by [`Previews::end`], as problems are of an open one.
    pub open: Option<Open>,
    /// How many previews were started: the last one's `started`.
    pub starts: u64,
    pub events: Vec<PreviewEvent>,
    /// The host's log mark already read.
    pub log_mark: u64,
    /// What was read for `problems` and is not yet among the events: lines
    /// join them when the events are read, so a reload's outcome comes
    /// before what was logged around it.
    unlisted: Vec<LogLine>,
    /// What is wrong in the open preview now, one a line: the compiler's
    /// words where it said any, else what running the line logged first.
    pub problems: Vec<LogLine>,
    /// Counts changes to `problems`.
    pub problems_seq: u64,
    /// Counts the previews that ended, for a call to see that it ended one.
    pub ends: u64,
    /// Each of `problems` with the text of its line when it was told, or
    /// `None` when that text could not be read. Neither the compiler nor
    /// the app says a problem again while its line is unchanged (Ljós tells
    /// a warning once; a reload keeps an object whose binding is the same),
    /// so a problem goes when its line's text does: an edit that leaves the
    /// line still wrong is told anew. One that names a file and no line of
    /// it is held by the whole file's text.
    held: Vec<(LogLine, Option<String>)>,
    /// The project's directory, where the open preview reads the files it
    /// runs; `None` for one held in no directory.
    pub root: Option<std::path::PathBuf>,
    pub timer: Arc<dyn Timer>,
}

/// The page's extent in whole pixels, as a preview is sized.
fn measure(host: &mut dyn PreviewHost) -> Result<Extent, PreviewError> {
    let mut extent = host.extent()?;
    extent.reach = extent.reach.ceil();
    Ok(extent)
}

/// Layouts a fit takes at most: a list that makes its delegates as they
/// come into view grows each time the preview does.
const FIT_ROUNDS: usize = 4;

/// Two lines about one place: one line of one file, or one message where no
/// line is told.
fn same_place(a: &LogLine, b: &LogLine) -> bool {
    let one_line = a.file.is_some() && a.line.is_some();
    a.file == b.file && a.line == b.line && (one_line || a.message == b.message)
}

/// A reload's failure as a problem, placed where its message says.
pub(crate) fn failure(message: &str, root: Option<&Path>) -> LogLine {
    LogLine::failure(message, root)
}

impl Open {
    /// Lays the preview out at its screen's size and, fitted to content,
    /// makes it as tall as what it then lays out, again while that grows.
    /// Measured at the screen's size each time, so a page that got shorter
    /// gets a shorter preview: an item that fills the window would
    /// otherwise always reach its bottom.
    pub fn refit(&mut self) -> Result<(), PreviewError> {
        let before = self.size;
        if self.size != self.screen {
            self.host.resize(self.screen)?;
            self.size = self.screen;
            self.measured = None;
        }
        self.grow(None)?;
        self.bumped(before);
        Ok(())
    }

    /// Opened again with its screen, fit and aliases as they were: one fitted
    /// to content keeps its height while what it lays out still reaches it,
    /// grows if that reaches further, and is laid out from its screen again
    /// only when that got shorter. An unchanged page is not resized at all.
    /// A root that fills the window is not what it lays out (see
    /// [`PreviewHost::extent`]), so a page that folds while it runs is
    /// measured shorter here. One that measures what it did when it was
    /// last grown, reaching as far and leaving out the same items, has not
    /// changed, though it is shorter than the preview: an item the host left
    /// out made the preview that tall. That item folded to within the rest
    /// reaches no further, but is no longer left out, so the page is laid
    /// out again. It grows from the measure it compared, so what it keeps
    /// is what it compared: a page that changes after is seen next time.
    pub fn keep_fit(&mut self) -> Result<(), PreviewError> {
        let mut compared = None;
        if self.fit == Fit::Content && self.size != self.screen {
            let now = measure(self.host.as_mut())?;
            if now.reach < f64::from(self.size.height) && self.measured.as_ref() != Some(&now) {
                return self.refit();
            }
            compared = Some(now);
        }
        let before = self.size;
        if self.fit == Fit::Screen && self.size != self.screen {
            return self.refit();
        }
        self.grow(compared)?;
        self.bumped(before);
        Ok(())
    }

    /// Fitted to content, makes the preview as tall as what it lays out at
    /// its present size, again while that grows; never shorter. What the
    /// page measured is forgotten only once a resize the host took changes
    /// what it means: a measure refused at the same height leaves it, so an
    /// unchanged page asked again is still not laid out from its screen.
    /// `first` is what the page measured at its present size just now,
    /// if it was: the first round takes it rather than measuring again.
    fn grow(&mut self, mut first: Option<Extent>) -> Result<(), PreviewError> {
        if self.fit != Fit::Content {
            self.measured = None;
            return Ok(());
        }
        for _ in 0..FIT_ROUNDS {
            let extent = match first.take() {
                Some(extent) => extent,
                None => measure(self.host.as_mut())?,
            };
            let reach = extent.reach;
            let height = if reach >= f64::from(Size::MAX_SIDE) {
                Size::MAX_SIDE
            } else {
                // Checked: below MAX_SIDE, so it fits in a u32.
                (reach.max(0.0) as u32).max(self.screen.height)
            };
            if height <= self.size.height {
                self.measured = Some(extent);
                break;
            }
            let size = Size {
                width: self.screen.width,
                height,
            };
            self.host.resize(size)?;
            self.size = size;
            self.measured = None;
        }
        Ok(())
    }

    /// After a fit or change the host refused partway, from `before`: the fit
    /// it had again. Back to fitting its screen, it is put back to its
    /// screen's size, or its fit would say screen while it is as tall as the
    /// page it grew to; a resize refused again is the next open's, which
    /// refits a preview not at its screen. A size it is left at that is not
    /// `before` changes its picture, and a window has to know to look.
    pub fn undo_fit(&mut self, was: Fit, before: Size) {
        self.fit = was;
        if was == Fit::Screen && self.size != self.screen && self.host.resize(self.screen).is_ok() {
            self.size = self.screen;
            self.measured = None;
        }
        self.bumped(before);
    }

    /// Its picture changed without a reload's generation, as a click's.
    fn bumped(&mut self, before: Size) {
        if self.size != before {
            self.clicks += 1;
        }
    }

    fn write_hidden(&mut self, ids: &BTreeSet<(String, String)>) {
        let mut wrote = false;
        for (_, id) in ids.iter().filter(|(doc, _)| *doc == self.doc) {
            // An object the text no longer has has nothing to hide.
            let _ = self.host.write(id, "visible", false.into());
            wrote = true;
        }
        // Drawn after the reload's picture was asked for, as a click is.
        if wrote {
            self.clicks += 1;
        }
    }

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
            starts: 0,
            events: Vec::new(),
            log_mark: 0,
            unlisted: Vec::new(),
            problems: Vec::new(),
            problems_seq: 0,
            ends: 0,
            held: Vec::new(),
            root: None,
            timer,
            hidden: BTreeSet::new(),
            hidden_ids: BTreeSet::new(),
        }
    }

    /// Whether the object with `handle` in `doc` is hidden.
    pub(crate) fn is_hidden(&self, doc: &str, handle: &str) -> bool {
        self.hidden.contains(&(doc.to_owned(), handle.to_owned()))
    }

    /// Hides an object, which has had `handles` and now has `id`, in the
    /// preview, now and after each reload.
    pub(crate) fn hide(&mut self, doc: &str, handles: BTreeSet<String>, id: String) {
        for handle in handles {
            self.hidden.insert((doc.to_owned(), handle));
        }
        self.hidden_ids.insert((doc.to_owned(), id));
        self.write_hidden();
    }

    /// Shows it again with `visible`, what its file says. The file has not
    /// changed, so a reload would change nothing: a `visible` the file binds
    /// to an expression holds this value until the next reload.
    pub(crate) fn show(&mut self, doc: &str, handles: BTreeSet<String>, id: &str, visible: bool) {
        let mut was = false;
        for handle in handles {
            was |= self.hidden.remove(&(doc.to_owned(), handle));
        }
        if !was {
            return;
        }
        self.hidden_ids.remove(&(doc.to_owned(), id.to_owned()));
        if let Some(open) = self.open.as_mut().filter(|open| open.doc == doc) {
            // An object the text no longer has has nothing to show.
            let _ = open.host.write(id, "visible", visible.into());
            open.clicks += 1;
        }
    }

    /// Holds the state the open preview's width turns on, when the file or
    /// the width has changed which that is. Whether anything was written.
    pub(crate) fn hold_breakpoint(&mut self, now: Option<(String, String)>) -> bool {
        let Some(open) = &mut self.open else {
            return false;
        };
        if open.breakpoint == now {
            return false;
        }
        let was = std::mem::replace(&mut open.breakpoint, now);
        if open.shown.is_some() {
            return false;
        }
        match (&open.breakpoint, was) {
            (Some((id, state)), _) => {
                for value in ["", state.as_str()] {
                    let _ = open.host.write(id, "state", value.into());
                }
            }
            (None, Some((id, _))) => {
                let _ = open.host.write(&id, "state", "".into());
            }
            (None, None) => {}
        }
        open.clicks += 1;
        true
    }

    /// What `node_arrange` `hide` holds, on what a reload or a new app put back.
    pub(crate) fn write_hidden(&mut self) {
        if let Some(open) = &mut self.open {
            open.write_hidden(&self.hidden_ids);
        }
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
        if let (Ok(Reload::Applied { .. }), Some((id, state))) =
            (&reload, open.shown.as_ref().or(open.breakpoint.as_ref()))
        {
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

        // Stamped once answered: however long the call took, the reload the
        // budget allows a second after this reaches the inspector more than
        // a second after this one did.
        open.reloads.push_back(self.timer.now());
        if matches!(reload, Ok(Reload::Applied { .. })) {
            open.write_hidden(&self.hidden_ids);
            // What the new text lays out may be taller or shorter. A refit
            // that fails (a call too many this second) leaves the last
            // height, and the next reload fits again.
            if open.fit == Fit::Content {
                let _ = open.refit();
            }
        }
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
                self.events.push(PreviewEvent::ReloadFailed {
                    message: e.to_string(),
                });
                return;
            }
        };
        let failed = match &event {
            PreviewEvent::ReloadFailed { message } => Some(failure(message, self.root.as_deref())),
            _ => None,
        };
        self.events.push(event);
        if let Ok(lines) = self.read_logs() {
            self.start_problems(failed, &lines);
        }
    }

    /// A new preview: nothing it logged has been read. The error is that it
    /// ended as that was read.
    pub fn opened(&mut self) -> Result<(), PreviewError> {
        self.log_mark = 0;
        self.held.clear();
        let lines = self.read_logs()?;
        self.start_problems(None, &lines);
        Ok(())
    }

    /// The text of `line` of `file` as the preview reads it now, or the
    /// file's where no line is told.
    fn text_at(&self, file: Option<&str>, line: Option<u32>) -> Option<String> {
        let text = std::fs::read_to_string(self.root.as_ref()?.join(file?)).ok()?;
        let Some(line) = line else {
            return Some(text);
        };
        let line = line.checked_sub(1)?;
        text.lines().nth(line as usize).map(|l| l.trim().to_owned())
    }

    /// After a reload. One that applied compiled, so no error of the
    /// compiler's outlasts it, whatever its line says; it keeps what else is
    /// wrong on lines it left as they were. One that failed keeps all of
    /// that, as the last good text runs on, with only its own failure for
    /// why the new text does not show.
    fn start_problems(&mut self, failed: Option<LogLine>, lines: &[LogLine]) {
        let held = std::mem::take(&mut self.held);
        self.held = held
            .into_iter()
            .filter(|(p, _)| !(p.compiler && p.level == "error"))
            .filter(|(p, text)| {
                failed.is_some()
                    || (text.is_some() && *text == self.text_at(p.file.as_deref(), p.line))
            })
            .collect();
        if let Some(failed) = failed {
            let text = self.text_at(failed.file.as_deref(), failed.line);
            self.held.push((failed, text));
        }
        self.add_problems(lines);
    }

    /// What was logged: the compiler's words win over what running the same
    /// line logged, whichever came first. Past [`LINES_KEPT`] the first
    /// problems stay and only the compiler's are added.
    fn add_problems(&mut self, lines: &[LogLine]) {
        for line in lines.iter().filter(|l| l.is_problem()) {
            let at = self.held.iter().position(|(p, _)| same_place(p, line));
            match at {
                Some(at) if line.compiler && !self.held[at].0.compiler => {
                    self.held[at].0 = line.clone();
                }
                Some(_) => {}
                None if line.compiler || self.held.len() < LINES_KEPT => {
                    let text = self.text_at(line.file.as_deref(), line.line);
                    self.held.push((line.clone(), text));
                }
                None => {}
            }
        }
        let now: Vec<LogLine> = self.held.iter().map(|(p, _)| p.clone()).collect();
        if now != self.problems {
            self.problems = now;
            self.problems_seq += 1;
        }
    }

    /// What a reload asked for at `started` (the timer's time) did. The
    /// error is that the preview ended as its logs were read.
    pub fn record(&mut self, reload: Reload, started: Duration) -> Result<(), PreviewError> {
        let failed = match reload {
            Reload::Applied { generation } => {
                self.events.push(PreviewEvent::Reloaded {
                    generation,
                    ms: ms(self.timer.now().saturating_sub(started)),
                });
                None
            }
            Reload::Failed { message } => {
                let failed = failure(&message, self.root.as_deref());
                self.events.push(PreviewEvent::ReloadFailed { message });
                Some(failed)
            }
            Reload::Unchanged => return Ok(()),
        };
        let lines = self.read_logs()?;
        self.start_problems(failed, &lines);
        Ok(())
    }

    /// Reads the host's new log lines, and puts all that are unlisted among
    /// the events.
    pub fn pull_logs(&mut self) {
        self.pull_problems();
        self.events
            .extend(self.unlisted.drain(..).map(PreviewEvent::Log));
    }

    /// Reads the host's new log lines for the problems among them. They
    /// join the events when those are read.
    pub fn pull_problems(&mut self) {
        if let Ok(lines) = self.read_logs() {
            self.add_problems(&lines);
        }
    }

    pub fn shown_now(&self) -> Shown {
        Shown {
            problems: self.problems_seq,
            open: self
                .open
                .as_ref()
                .map(|o| (o.doc.clone(), o.target, o.size, o.state, o.clicks)),
        }
    }

    /// The host's new log lines, kept for the events. The error is that
    /// the preview ended as they were read: its problems went with it, and
    /// none are to be put back.
    fn read_logs(&mut self) -> Result<Vec<LogLine>, PreviewError> {
        let Some(open) = &mut self.open else {
            return Ok(Vec::new());
        };
        match open.host.logs(self.log_mark) {
            Ok((lines, next)) => {
                self.log_mark = next;
                self.unlisted.extend_from_slice(&lines);
                let over = self.unlisted.len().saturating_sub(LINES_KEPT);
                self.unlisted.drain(..over);
                Ok(lines)
            }
            Err(e @ (PreviewError::Ended(_) | PreviewError::NotOpen)) => Err(self.failed(e)),
            Err(e) => {
                open.behind = true;
                self.events.push(PreviewEvent::ReloadFailed {
                    message: e.to_string(),
                });
                Ok(Vec::new())
            }
        }
    }

    /// Ends the preview, however it came to: no preview runs, so nothing is
    /// wrong in one. The preview that was open is the caller's to close.
    pub fn end(&mut self) -> Option<Open> {
        self.held.clear();
        self.unlisted.clear();
        if !self.problems.is_empty() {
            self.problems.clear();
            self.problems_seq += 1;
        }
        let open = self.open.take();
        self.ends += u64::from(open.is_some());
        open
    }

    /// What every host call's error goes through: one that says the preview
    /// is over ends it here too, or the next `preview_open` would reuse a
    /// process that is gone. Any other leaves it open.
    pub fn failed(&mut self, e: PreviewError) -> PreviewError {
        if matches!(e, PreviewError::Ended(_) | PreviewError::NotOpen) {
            self.end();
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
    use studio_core::preview::{Declared, Picture, PreviewError};

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
        fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
            Ok((Vec::new(), since))
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
            hidden_ids: BTreeSet::new(),
            open: Some(Open {
                started: 1,
                host: Box::new(Counting(count.clone())),
                doc: "Main.qml".to_owned(),
                target: None,
                size: Target::Desktop.size(),
                screen: Target::Desktop.size(),
                fit: Fit::Screen,
                measured: None,
                state: State::Live,
                mocked: Vec::new(),
                reloads,
                behind,
                clicks: 0,
                shown: None,
                breakpoint: None,
                harness: None,
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

    fn warning(file: &str, line: u32, message: &str, compiler: bool) -> LogLine {
        LogLine {
            level: "warning".to_owned(),
            message: message.to_owned(),
            file: Some(file.to_owned()),
            line: Some(line),
            compiler,
        }
    }

    /// A project of one file, written by the test as an edit writes it.
    fn rooted(p: &mut Previews, text: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "studio-problems-{}-{}",
            std::process::id(),
            text.len()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Main.qml"), text).unwrap();
        p.root = Some(root.clone());
        root
    }

    #[test]
    fn a_problem_stays_while_its_line_is_unchanged_and_goes_when_edited() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let root = rooted(&mut p, "Item {\n    color: transparent\n    width: 1\n}\n");
        let said = warning("Main.qml", 2, "`transparent` is not a name in sight", true);
        let ran = warning(
            "Main.qml",
            2,
            "Unable to assign [undefined] to QColor",
            false,
        );
        // What running the line logged comes first; the compiler's words win.
        p.start_problems(None, &[ran.clone(), said.clone()]);
        assert_eq!(p.problems, std::slice::from_ref(&said));
        let seq = p.problems_seq;
        // Another line edited: nobody says it again, and it is still wrong.
        std::fs::write(
            root.join("Main.qml"),
            "Item {\n    color: transparent\n    width: 2\n}\n",
        )
        .unwrap();
        p.start_problems(None, &[]);
        assert_eq!(p.problems, [said]);
        assert_eq!(p.problems_seq, seq, "nothing changed");
        // Its own line edited and fixed: it goes.
        std::fs::write(
            root.join("Main.qml"),
            "Item {\n    color: \"transparent\"\n    width: 2\n}\n",
        )
        .unwrap();
        p.start_problems(None, &[]);
        assert!(p.problems.is_empty());
        assert_eq!(p.problems_seq, seq + 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_problem_whose_line_cannot_be_read_lasts_one_reload() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let ran = warning(
            "ljos-design:/Ljos/Design/Colors.qml",
            93,
            "Unable to assign [undefined] to QColor",
            false,
        );
        p.start_problems(None, std::slice::from_ref(&ran));
        assert_eq!(p.problems, std::slice::from_ref(&ran));
        p.start_problems(None, &[]);
        assert!(p.problems.is_empty());
    }

    #[test]
    fn a_failed_reload_keeps_what_was_wrong_and_adds_why() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let ran = warning("Main.qml", 5, "ReferenceError: x is not defined", false);
        p.start_problems(None, std::slice::from_ref(&ran));
        p.start_problems(
            Some(failure("Main.qml:9:3: expected a property", None)),
            &[],
        );
        assert_eq!(p.problems.len(), 2);
        assert_eq!(p.problems[1].line, Some(9));
        assert_eq!(p.problems[1].message, "expected a property");
        assert_eq!(p.problems[1].level, "error");
        // The next failure says why instead of the last.
        p.start_problems(Some(failure("Main.qml:4:1: expected `}`", None)), &[]);
        assert_eq!(p.problems.len(), 2);
        assert_eq!(p.problems[1].line, Some(4));
    }

    #[test]
    fn an_error_goes_when_a_reload_applies_though_its_line_is_as_it_was() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let root = rooted(&mut p, "Item {\n    Badge {}\n}\n");
        p.start_problems(
            Some(failure("Main.qml:2:5: Badge is not a type", None)),
            &[],
        );
        assert_eq!(p.problems[0].level, "error");
        // Mended by a new Badge.qml: line 2 is untouched.
        p.start_problems(None, &[]);
        assert!(p.problems.is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_script_line_is_not_given_as_the_documents() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let root = rooted(&mut p, "Item { id: scripted }\n");
        // Measured at the pin for line 15 of a document under "my pages".
        let placed = failure("Main.qml.js:21: TypeError: x", p.root.as_deref());
        assert_eq!(placed.file.as_deref(), Some("Main.qml"));
        assert_eq!(placed.line, None);
        assert_eq!(placed.message, "TypeError: x");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn warnings_of_a_file_without_a_line_are_each_held_by_its_text() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let root = rooted(&mut p, "Item { id: two_warnings }\n");
        let said = ["TypeError: x", "TypeError: y"].map(|message| LogLine {
            level: "warning".to_owned(),
            message: message.to_owned(),
            file: Some("Main.qml".to_owned()),
            line: None,
            compiler: false,
        });
        p.start_problems(None, &said);
        assert_eq!(p.problems, said);
        // Another file was edited.
        p.start_problems(None, &[]);
        assert_eq!(p.problems, said);
        std::fs::write(root.join("Main.qml"), "Item {}\n").unwrap();
        p.start_problems(None, &[]);
        assert!(p.problems.is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failure_names_the_project_file_it_is_in() {
        let (mut p, _) = previews(VecDeque::new(), false);
        let root = rooted(&mut p, "Item {}\n");
        std::fs::create_dir_all(root.join("my pages")).unwrap();
        std::fs::write(root.join("my pages/Page.qml"), "Item {\n").unwrap();
        let mut said = |message: &str| {
            p.record(
                Reload::Failed {
                    message: message.to_owned(),
                },
                Duration::ZERO,
            )
            .unwrap();
            assert_eq!(p.problems.len(), 1, "{:?}", p.problems);
            let said = p.problems[0].clone();
            (said.file, said.line, said.message)
        };
        // Ljós writes no line where the whole file is at fault.
        assert_eq!(
            said("Main.qml: no entry has a file"),
            (
                Some("Main.qml".to_owned()),
                None,
                "no entry has a file".to_owned()
            )
        );
        assert_eq!(
            said("my pages/Page.qml:3:1: expected `}`"),
            (
                Some("my pages/Page.qml".to_owned()),
                Some(3),
                "expected `}`".to_owned()
            )
        );
        // Words before a colon that are no file of the project are a sentence.
        let sentence = "cannot load Main.qml: no such type";
        assert_eq!(said(sentence), (None, None, sentence.to_owned()));
        std::fs::remove_dir_all(&root).unwrap();
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
