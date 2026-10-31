//! Previews: a document of the project running as the shipped app runs it.
//!
//! A preview runs user code, so it lives out of Studio's process (a child
//! Ljós application natively, a sandboxed frame on the web) behind
//! [`PreviewHost`]; a hang or crash there must not take Studio down
//! (DESIGN.md, section 2). The session does not own one: whoever shows the
//! preview (the MCP server, the canvas) holds the host and tells it after
//! each write, which is what keeps the edit-to-preview path free of the
//! hot-reload watcher's quiet time.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The shape of screen a preview is laid out for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Desktop,
    Tablet,
    Phone,
}

impl Target {
    /// Logical pixels, portrait for the hand-held ones: sizes common enough
    /// that a design checked at them holds on most devices of the kind.
    pub fn size(self) -> Size {
        match self {
            Target::Desktop => Size {
                width: 1280,
                height: 800,
            },
            Target::Tablet => Size {
                width: 820,
                height: 1180,
            },
            Target::Phone => Size {
                width: 390,
                height: 844,
            },
        }
    }
}

impl Target {
    pub const ALL: [Target; 3] = [Target::Phone, Target::Tablet, Target::Desktop];

    /// The widths a layout for this target is for: from the first
    /// (inclusive) up to the second. The bounds sit between the targets'
    /// own widths where common devices split: phones held upright are
    /// narrower than 600, tablets upright and small laptops narrower than
    /// 1024.
    pub fn widths(self) -> (Option<u32>, Option<u32>) {
        match self {
            Target::Phone => (None, Some(600)),
            Target::Tablet => (Some(600), Some(1024)),
            Target::Desktop => (Some(1024), None),
        }
    }

    /// The target whose widths hold `width`.
    pub fn of_width(width: u32) -> Target {
        Target::ALL
            .into_iter()
            .find(|t| {
                let (from, to) = t.widths();
                from.is_none_or(|f| width >= f) && to.is_none_or(|t| width < t)
            })
            .expect("the targets' widths cover every width")
    }

    /// The `when` of a state for this target: `root`'s width within
    /// [`widths`](Self::widths), where `root` is the root object's `id`.
    pub fn condition(self, root: &str) -> String {
        match self.widths() {
            (None, Some(to)) => format!("{root}.width < {to}"),
            (Some(from), Some(to)) => format!("{root}.width >= {from} && {root}.width < {to}"),
            (Some(from), None) => format!("{root}.width >= {from}"),
            (None, None) => "true".to_owned(),
        }
    }

    /// The target whose [`condition`](Self::condition) `when` is, as
    /// Studio writes it.
    pub fn of_condition(when: &str, root: &str) -> Option<Target> {
        Target::ALL
            .into_iter()
            .find(|t| t.condition(root) == when.trim())
    }
}

/// Logical pixels, 1 to 8192 on each side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Size {
    #[schemars(range(min = 1, max = Size::MAX_SIDE))]
    pub width: u32,
    #[schemars(range(min = 1, max = Size::MAX_SIDE))]
    pub height: u32,
}

impl Size {
    /// The longest side a preview is laid out for: past every screen the
    /// targets stand for, and short of a headless surface large enough to
    /// matter for memory.
    pub const MAX_SIDE: u32 = 8192;

    /// Whether a preview can be laid out at this size.
    pub fn fits(self) -> bool {
        let side = 1..=Self::MAX_SIDE;
        side.contains(&self.width) && side.contains(&self.height)
    }
}

/// One module standing in for another: `module` is imported from `dir`
/// (relative to the project) instead of where it would be found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Alias {
    pub module: String,
    pub dir: String,
}

/// What to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewSpec {
    /// The document, as the store names it (`Shell.qml`).
    pub entry: String,
    pub size: Size,
    pub aliases: Vec<Alias>,
    /// The project's type dumps (`ljostypes.json`), relative to it: the
    /// types an application registers from Rust, which the preview stands
    /// in for with the members they declare.
    pub types: Vec<String>,
}

/// What a reload did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum Reload {
    /// The preview shows the files now; `generation` counts reloads.
    Applied { generation: u64 },
    /// The files did not compile; the preview goes on showing the last
    /// good generation.
    Failed { message: String },
    /// Nothing the preview runs had changed.
    Unchanged,
}

/// A line the previewed app logged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LogLine {
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// A PNG of the preview.
#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("bytes", &self.png.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

/// A place in a file of the project where an object of the preview is
/// declared: the start of its type name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    /// As the store names it (`Main.qml`, `App/Media/Player.qml`), or as the
    /// preview gave it when it is not a file of the project.
    pub file: String,
    /// One-based.
    pub line: u32,
    /// One-based, in characters; 0 when only the line is known.
    pub column: u32,
}

/// An object of the running preview: where it is and where it was declared.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// In its parent's coordinates.
    pub local: crate::place::Rect,
    /// In the window's.
    pub window: crate::place::Rect,
    /// None for an object no file declares.
    pub declared: Option<Declared>,
    /// Its type and those it derives from.
    pub types: Vec<String>,
}

/// What a click in the preview is aimed at.
#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    /// The object with this `id`.
    Id(String),
    /// This point, in the preview's logical pixels: whatever is on top
    /// there gets the click.
    Point { x: f64, y: f64 },
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("no preview is open")]
    NotOpen,
    /// The preview could not be started (no runtime, the entry does not
    /// compile, the process ended before it answered).
    #[error("the preview did not start: {0}")]
    Start(String),
    /// The preview's process ended; `preview_open` starts another.
    #[error("the preview has ended: {0}")]
    Ended(String),
    /// The preview has no object of that name.
    #[error("the preview has no object `{0}`")]
    NoObject(String),
    /// The preview refused, with its own error code (`invalid_argument`,
    /// `app_faulted`…) and message.
    #[error("the preview refused ({code}): {message}")]
    Refused { code: String, message: String },
    /// The preview's runtime cannot answer this call yet (a browser preview
    /// on ljos-web), and which numbered ljos-web request it waits on
    /// (DESIGN.md, S5). Never a made-up answer in its place.
    #[error(
        "the preview cannot do this yet: {message} (waits on ljos-web request {ljos_web_request})"
    )]
    NotSupported {
        message: String,
        ljos_web_request: u32,
    },
    #[error("the preview did not answer: {0}")]
    Transport(String),
}

/// A running preview.
///
/// Objects are named by their `id` in the entry document, which the
/// session's block inserts always give; a host maps that to its own way of
/// finding an object.
/// The calls a second a Ljós inspector answers unless its launcher grants
/// more (Ljós ARCHITECTURE.md 19.7).
pub const DEFAULT_CALLS_PER_SECOND: u32 = 20;

/// What a preview said when its UI thread last went to sleep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rest {
    /// How many times it has gone to sleep saying something new.
    pub rests: u64,
    /// When it wakes by itself next; `None` for not until something happens.
    pub wake_in_ms: Option<u64>,
}

/// Waits on a running preview from outside the session's lock, so a long
/// wait holds up nobody.
pub trait RestWatch: Send {
    /// Answers at once without `since`, else when `rests` differs from `since`
    /// or after `wait_ms`.
    fn wait(&self, since: Option<u64>, wait_ms: u64) -> Result<Rest, PreviewError>;
}

pub trait PreviewHost: Send {
    /// The calls a second the preview answers before it asks a caller to
    /// wait: what bounds how often Studio may ask it to reload.
    fn calls_per_second(&self) -> u32 {
        DEFAULT_CALLS_PER_SECOND
    }

    /// Starts the preview of `spec`, ending one that runs.
    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError>;

    /// Shows the files as they are now, at once: called after each write.
    fn reload(&mut self) -> Result<Reload, PreviewError>;

    /// Imports `module` from `dir` (or, with `None`, from where it would be
    /// found again) and reloads, keeping state.
    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError>;

    fn resize(&mut self, size: Size) -> Result<(), PreviewError>;

    /// A picture whose longest side is at most `max_side`.
    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError>;

    /// What the preview shows as text, one line per item, for reading
    /// rather than looking.
    fn outline(&mut self) -> Result<String, PreviewError>;

    /// The values of `names` on the object with `id`.
    fn read(
        &mut self,
        id: &str,
        names: &[String],
    ) -> Result<serde_json::Map<String, serde_json::Value>, PreviewError>;

    /// Assigns `value` to `name` on the object with `id`, as a script would.
    fn write(&mut self, id: &str, name: &str, value: serde_json::Value)
    -> Result<(), PreviewError>;

    /// Log lines after the mark `since` (0: all kept), and the mark to pass
    /// next.
    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError>;

    /// Whether the preview still runs: `Ended` (or `NotOpen`) when it has
    /// stopped since the last call. Cheap and local where the host can
    /// tell without asking the preview, since every reopen asks it.
    fn alive(&mut self) -> Result<(), PreviewError>;

    /// Where the object the preview shows at `x`, `y` (its logical pixels)
    /// is declared: the topmost object there that has a declaration.
    /// `None` when nothing is there.
    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError>;

    /// The objects with one of `ids` and, among objects without an id, those
    /// of one of `types`, in scene order. Copies of a delegate are each
    /// listed. A host that cannot tell refuses.
    fn placed(&mut self, ids: &[String], types: &[String]) -> Result<Vec<Placed>, PreviewError> {
        let _ = (ids, types);
        Err(PreviewError::Refused {
            code: "unsupported".to_owned(),
            message: "this preview does not say where its objects are".to_owned(),
        })
    }

    /// Clicks as a person does, with the primary button: the app's own
    /// handlers answer it. A host that takes no input refuses.
    fn click(&mut self, at: &Click) -> Result<(), PreviewError> {
        let _ = at;
        Err(PreviewError::Refused {
            code: "unsupported".to_owned(),
            message: "this preview takes no input".to_owned(),
        })
    }

    /// Ends the preview.
    fn close(&mut self);

    /// A way to wait on this preview; `None` when it cannot tell.
    fn rest_watch(&self) -> Option<Box<dyn RestWatch>> {
        None
    }
}

/// Something that happened to a preview, in the order it happened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PreviewEvent {
    Opened {
        entry: String,
        size: Size,
        mocked: Vec<String>,
        ms: f64,
    },
    Reloaded {
        generation: u64,
        ms: f64,
    },
    ReloadFailed {
        message: String,
    },
    Log(LogLine),
    Ended {
        message: String,
    },
}

/// Milliseconds, as events and answers carry them.
pub fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 1e4).round() / 10.0
}
