//! What Studio and a preview page say to each other.
//!
//! A preview on the web is an ljos-web runtime in a sandboxed frame (or a
//! worker): it runs the project's code, so it lives apart from Studio as a
//! child process does natively (DESIGN.md section 2). The two talk by
//! `postMessage`, one [`Request`] and one [`Answer`] per call of
//! [`PreviewHost`](studio_core::preview::PreviewHost), as JSON:
//!
//! ```json
//! {"ljos-preview": 1, "id": 7, "request": {"call": "reload", "changed": {"Main.qml": "…"}}}
//! {"ljos-preview": 1, "id": 7, "answer": {"answer": "reload", "outcome": "applied", "generation": 2}}
//! ```
//!
//! The page has no access to Studio's storage (a sandboxed frame has an
//! origin of its own), so Studio sends the files: all of them on `open`,
//! then the ones that changed on `reload` and `alias`. `web/preview.js` is
//! the JavaScript side of these types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use studio_core::preview::{Alias, LogLine, Reload, Size};

/// The key every message of this protocol carries, with [`VERSION`]: a
/// frame hears other messages too.
pub const TAG: &str = "ljos-preview";
pub const VERSION: u32 = 1;

/// One call of the host, as the page is asked it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum Request {
    /// Run `entry` at `size`, with each alias's module imported from its
    /// directory; `files` is the whole project.
    Open {
        entry: String,
        size: Size,
        aliases: Vec<Alias>,
        files: BTreeMap<String, String>,
    },
    /// Show the files as they are now: `changed` has each file's new text,
    /// or `null` for one removed.
    Reload {
        changed: BTreeMap<String, Option<String>>,
    },
    /// Import `module` from `dir` (`null`: from where it would be found)
    /// and reload with `changed`, keeping state.
    Alias {
        module: String,
        dir: Option<String>,
        changed: BTreeMap<String, Option<String>>,
    },
    Resize {
        size: Size,
    },
    /// A PNG at most `max_side` pixels on its longest side.
    Screenshot {
        max_side: u32,
    },
    Outline,
    Read {
        id: String,
        names: Vec<String>,
    },
    Write {
        id: String,
        name: String,
        value: Value,
    },
    /// Log lines after the mark `since`.
    Logs {
        since: u64,
    },
    /// Where the object at `x`, `y` (logical pixels) is declared.
    Pick {
        x: f64,
        y: f64,
    },
    Close,
}

impl Request {
    /// The call's name, as the page's refusals and Studio's errors say it.
    pub fn name(&self) -> &'static str {
        match self {
            Request::Open { .. } => "open",
            Request::Reload { .. } => "reload",
            Request::Alias { .. } => "alias",
            Request::Resize { .. } => "resize",
            Request::Screenshot { .. } => "screenshot",
            Request::Outline => "outline",
            Request::Read { .. } => "read",
            Request::Write { .. } => "write",
            Request::Logs { .. } => "logs",
            Request::Pick { .. } => "pick",
            Request::Close => "close",
        }
    }
}

/// Where an object is declared: the start of its type name, as
/// [`Declared`](studio_core::preview::Declared) says it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub file: String,
    /// One-based.
    pub line: u32,
    /// One-based, in characters; 0 when only the line is known.
    pub column: u32,
}

/// The page's answer to one [`Request`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "snake_case")]
pub enum Answer {
    /// `open`, `resize`, `write` and `close` did what they were asked.
    Done,
    /// What `reload` or `alias` did.
    Reload(Reload),
    /// `png` is base64.
    Picture {
        png: String,
        width: u32,
        height: u32,
    },
    Outline {
        text: String,
    },
    Values {
        values: Map<String, Value>,
    },
    Logs {
        lines: Vec<LogLine>,
        next: u64,
    },
    Picked {
        declared: Option<Place>,
    },
    /// `open` could not start the preview (the entry does not compile, the
    /// page lacks something it needs).
    NotStarted {
        message: String,
    },
    /// The preview has no object `id`.
    NoObject {
        id: String,
    },
    /// The page cannot do what it was asked, with an error code of its own
    /// (`unsupported` for a call this page does not do yet).
    Refused {
        code: String,
        message: String,
    },
}

impl Answer {
    pub fn name(&self) -> &'static str {
        match self {
            Answer::Done => "done",
            Answer::Reload(_) => "reload",
            Answer::Picture { .. } => "picture",
            Answer::Outline { .. } => "outline",
            Answer::Values { .. } => "values",
            Answer::Logs { .. } => "logs",
            Answer::Picked { .. } => "picked",
            Answer::NotStarted { .. } => "not_started",
            Answer::NoObject { .. } => "no_object",
            Answer::Refused { .. } => "refused",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requests_and_answers_read_as_the_page_writes_them() {
        let request = Request::Reload {
            changed: [
                ("Main.qml".to_owned(), Some("Item {}\n".to_owned())),
                ("Old.qml".to_owned(), None),
            ]
            .into(),
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({"call": "reload", "changed": {"Main.qml": "Item {}\n", "Old.qml": null}})
        );
        let answer: Answer = serde_json::from_value(
            json!({"answer": "reload", "outcome": "applied", "generation": 2}),
        )
        .unwrap();
        assert_eq!(answer, Answer::Reload(Reload::Applied { generation: 2 }));
        let answer: Answer =
            serde_json::from_value(json!({"answer": "picked", "declared": null})).unwrap();
        assert_eq!(answer, Answer::Picked { declared: None });
        assert_eq!(
            serde_json::to_value(Request::Outline).unwrap(),
            json!({"call": "outline"})
        );
    }
}
