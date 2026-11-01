//! The preview host on the web: Studio's side of [`protocol`](crate::protocol).
//!
//! [`PreviewHost`] answers now, while a page answers a message later. The
//! [`Port`] hides that: natively a test's fake page answers in the call; in
//! a browser Studio runs in a worker, and its port posts the request to the
//! page and waits on a shared word for the answer (`web/preview.js`), so a
//! preview that hangs times out there instead of hanging Studio.

use std::collections::BTreeMap;

use base64::Engine as _;
use serde_json::{Map, Value};
use studio_core::preview::{
    Click, Declared, LogLine, Picture, PreviewError, PreviewHost, PreviewSpec, Reload, Size,
};

use crate::{
    WebStore,
    protocol::{Answer, Request},
};

/// The calls a second a preview page is said to answer.
///
/// DECISION: 150. The bridge has no limiter and no `Retry-After` to stay
/// under (each call blocks the worker until the page answers), so this is
/// only Studio's own pace: two fifths of it are reloads in a burst, 60 a
/// second, one a display frame at 60 Hz. More than Ljós's inspector default
/// (20), which would show a person's typing 8 times a second; less than
/// the native preview's 200, because a page's preview draws on the thread
/// the shell draws on, so a reload no frame can show costs the shell a
/// frame, where natively it costs another process.
pub const CALLS_PER_SECOND: u32 = 150;

/// Why a page gave no answer.
#[derive(Debug, thiserror::Error)]
pub enum PortError {
    /// It did not answer in time, or answered with something that is not
    /// an answer of this protocol.
    #[error("{0}")]
    NoAnswer(String),
    /// The page is gone (its frame was removed or reloaded).
    #[error("{0}")]
    Gone(String),
}

/// One request to the page and its answer.
pub trait Port: Send {
    fn ask(&mut self, request: &Request) -> Result<Answer, PortError>;
    /// Why the page is gone, if it is, from what this side already knows
    /// (a closed channel, a removed frame): never a request, so asking is
    /// free when the preview has ended.
    fn gone(&self) -> Option<String>;
}

/// A preview page, given the project's files over a [`Port`].
pub struct WebPreviewHost<P> {
    port: P,
    store: WebStore,
    /// What the page was last given of each file, so a reload sends only
    /// what changed; `None` while no preview is open.
    sent: Option<BTreeMap<String, String>>,
}

impl<P: Port> WebPreviewHost<P> {
    pub fn new(port: P, store: WebStore) -> Self {
        Self {
            port,
            store,
            sent: None,
        }
    }

    fn ask(&mut self, request: Request) -> Result<Answer, PreviewError> {
        if self.sent.is_none() && !matches!(request, Request::Open { .. }) {
            return Err(PreviewError::NotOpen);
        }
        let call = request.name();
        match self.port.ask(&request) {
            Ok(Answer::Refused { code, message }) => Err(PreviewError::Refused { code, message }),
            Ok(Answer::NotSupported {
                message,
                ljos_web_request,
            }) => Err(PreviewError::NotSupported {
                message,
                ljos_web_request,
            }),
            Ok(Answer::NoObject { id }) => Err(PreviewError::NoObject(id)),
            Ok(answer) => Ok(answer),
            Err(PortError::NoAnswer(message)) => {
                Err(PreviewError::Transport(format!("{call}: {message}")))
            }
            Err(PortError::Gone(message)) => {
                self.sent = None;
                Err(PreviewError::Ended(message))
            }
        }
    }

    /// The files that differ from what the page has, and the page's copy
    /// as it will be once it has them.
    fn changes(&self) -> (BTreeMap<String, Option<String>>, BTreeMap<String, String>) {
        let now = self.store.files();
        let before = self.sent.as_ref().expect("checked: a preview is open");
        let mut changed: BTreeMap<String, Option<String>> = now
            .iter()
            .filter(|(path, text)| before.get(*path) != Some(*text))
            .map(|(path, text)| (path.clone(), Some(text.clone())))
            .collect();
        changed.extend(
            before
                .keys()
                .filter(|path| !now.contains_key(*path))
                .map(|path| (path.clone(), None)),
        );
        (changed, now)
    }

    fn reloaded(
        &mut self,
        answer: Answer,
        now: BTreeMap<String, String>,
    ) -> Result<Reload, PreviewError> {
        match answer {
            Answer::Reload(reload) => {
                // A failed reload still left the page holding the files.
                self.sent = Some(now);
                Ok(reload)
            }
            other => Err(unexpected("reload", &other)),
        }
    }
}

fn unexpected(call: &str, answer: &Answer) -> PreviewError {
    PreviewError::Transport(format!("{call}: the page answered `{}`", answer.name()))
}

impl<P: Port> PreviewHost for WebPreviewHost<P> {
    fn restarts_on_switch(&self) -> bool {
        true
    }

    fn calls_per_second(&self) -> u32 {
        CALLS_PER_SECOND
    }

    fn click(&mut self, at: &Click) -> Result<(), PreviewError> {
        let request = match at {
            Click::Id(id) => Request::Click {
                id: Some(id.clone()),
                at: None,
            },
            Click::Point { x, y } => Request::Click {
                id: None,
                at: Some([*x, *y]),
            },
        };
        match self.ask(request)? {
            Answer::Done => Ok(()),
            other => Err(unexpected("click", &other)),
        }
    }

    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError> {
        let files = self.store.files();
        if !files.contains_key(&spec.entry) {
            return Err(PreviewError::Start(format!("no document `{}`", spec.entry)));
        }
        self.sent = None;
        let answer = self.ask(Request::Open {
            entry: spec.entry.clone(),
            size: spec.size,
            aliases: spec.aliases.clone(),
            files: files.clone(),
        })?;
        match answer {
            Answer::Done => {
                self.sent = Some(files);
                Ok(())
            }
            Answer::NotStarted { message } => Err(PreviewError::Start(message)),
            other => Err(unexpected("open", &other)),
        }
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        if self.sent.is_none() {
            return Err(PreviewError::NotOpen);
        }
        let (changed, now) = self.changes();
        if changed.is_empty() {
            return Ok(Reload::Unchanged);
        }
        let answer = self.ask(Request::Reload { changed })?;
        self.reloaded(answer, now)
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        if self.sent.is_none() {
            return Err(PreviewError::NotOpen);
        }
        let (changed, now) = self.changes();
        let answer = self.ask(Request::Alias {
            module: module.to_owned(),
            dir: dir.map(str::to_owned),
            changed,
        })?;
        self.reloaded(answer, now)
    }

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        match self.ask(Request::Resize { size })? {
            Answer::Done => Ok(()),
            other => Err(unexpected("resize", &other)),
        }
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        match self.ask(Request::Screenshot { max_side })? {
            Answer::Picture { png, width, height } => {
                let png = base64::engine::general_purpose::STANDARD
                    .decode(png)
                    .map_err(|e| PreviewError::Transport(format!("screenshot: {e}")))?;
                Ok(Picture { png, width, height })
            }
            other => Err(unexpected("screenshot", &other)),
        }
    }

    fn outline(&mut self) -> Result<String, PreviewError> {
        match self.ask(Request::Outline)? {
            Answer::Outline { text } => Ok(text),
            other => Err(unexpected("outline", &other)),
        }
    }

    fn read(&mut self, id: &str, names: &[String]) -> Result<Map<String, Value>, PreviewError> {
        match self.ask(Request::Read {
            id: id.to_owned(),
            names: names.to_vec(),
        })? {
            Answer::Values { values } => Ok(values),
            other => Err(unexpected("read", &other)),
        }
    }

    fn write(&mut self, id: &str, name: &str, value: Value) -> Result<(), PreviewError> {
        match self.ask(Request::Write {
            id: id.to_owned(),
            name: name.to_owned(),
            value,
        })? {
            Answer::Done => Ok(()),
            other => Err(unexpected("write", &other)),
        }
    }

    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
        match self.ask(Request::Logs { since })? {
            Answer::Logs { lines, next } => Ok((lines, next)),
            other => Err(unexpected("logs", &other)),
        }
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        match self.ask(Request::Pick { x, y })? {
            Answer::Picked { declared } => Ok(declared.map(|p| Declared {
                file: p.file,
                line: p.line,
                column: p.column,
            })),
            other => Err(unexpected("pick", &other)),
        }
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        if self.sent.is_none() {
            return Err(PreviewError::NotOpen);
        }
        match self.port.gone() {
            Some(message) => {
                self.sent = None;
                Err(PreviewError::Ended(message))
            }
            None => Ok(()),
        }
    }

    fn close(&mut self) {
        if self.sent.take().is_some() {
            // The page may already be gone; closed is closed either way.
            let _ = self.port.ask(&Request::Close);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::protocol::Place;

    /// A page that keeps the files it is given and answers from a script.
    #[derive(Clone, Default)]
    pub(crate) struct FakePage {
        pub heard: Arc<Mutex<Vec<Request>>>,
        pub files: Arc<Mutex<BTreeMap<String, String>>>,
        pub gone: Arc<Mutex<bool>>,
    }

    impl Port for FakePage {
        fn ask(&mut self, request: &Request) -> Result<Answer, PortError> {
            if *self.gone.lock().unwrap() {
                return Err(PortError::Gone("the frame was removed".into()));
            }
            self.heard.lock().unwrap().push(request.clone());
            let mut files = self.files.lock().unwrap();
            Ok(match request {
                Request::Open { files: given, .. } => {
                    *files = given.clone();
                    Answer::Done
                }
                Request::Reload { changed } | Request::Alias { changed, .. } => {
                    for (path, text) in changed {
                        match text {
                            Some(text) => files.insert(path.clone(), text.clone()),
                            None => files.remove(path),
                        };
                    }
                    let generation = self
                        .heard
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|r| matches!(r, Request::Reload { .. } | Request::Alias { .. }))
                        .count() as u64;
                    Answer::Reload(Reload::Applied { generation })
                }
                Request::Outline => Answer::Outline {
                    text: files.keys().cloned().collect::<Vec<_>>().join("\n"),
                },
                Request::Read { id, .. } if id == "nobody" => Answer::NoObject { id: id.clone() },
                Request::Read { names, .. } => Answer::Values {
                    values: names.iter().map(|n| (n.clone(), json!(1))).collect(),
                },
                Request::Screenshot { .. } => Answer::Picture {
                    png: "iVBORw==".into(),
                    width: 2,
                    height: 1,
                },
                Request::Pick { x, .. } if *x < 0.0 => Answer::Picked { declared: None },
                Request::Pick { .. } => Answer::Picked {
                    declared: Some(Place {
                        file: "Main.qml".into(),
                        line: 3,
                        column: 1,
                    }),
                },
                Request::Logs { since } => Answer::Logs {
                    lines: (*since..1)
                        .map(|_| LogLine {
                            level: "info".into(),
                            message: "ready".into(),
                            file: None,
                            line: None,
                        })
                        .collect(),
                    next: 1,
                },
                Request::Write { .. } => Answer::NotSupported {
                    message: "this page cannot write properties".into(),
                    ljos_web_request: 4,
                },
                Request::Click {
                    id: Some(nobody), ..
                } if nobody == "nobody" => Answer::NoObject { id: nobody.clone() },
                Request::Click { .. } | Request::Resize { .. } | Request::Close => Answer::Done,
            })
        }

        fn gone(&self) -> Option<String> {
            self.gone
                .lock()
                .unwrap()
                .then(|| "the frame was removed".to_owned())
        }
    }

    fn host() -> (WebPreviewHost<FakePage>, FakePage, WebStore) {
        let store = WebStore::new(
            "memory",
            [
                ("Main.qml".to_owned(), "Item {}\n".to_owned()),
                ("Other.qml".to_owned(), "Text {}\n".to_owned()),
            ],
        );
        let page = FakePage::default();
        (
            WebPreviewHost::new(page.clone(), store.clone()),
            page,
            store,
        )
    }

    fn spec() -> PreviewSpec {
        PreviewSpec {
            entry: "Main.qml".into(),
            size: Size {
                width: 390,
                height: 844,
            },
            aliases: vec![],
            types: vec![],
        }
    }

    #[test]
    fn nothing_is_asked_before_a_preview_opens() {
        let (mut host, page, _) = host();
        assert!(matches!(host.reload(), Err(PreviewError::NotOpen)));
        assert!(matches!(host.outline(), Err(PreviewError::NotOpen)));
        host.close();
        assert!(page.heard.lock().unwrap().is_empty());
    }

    #[test]
    fn the_page_is_given_the_project_then_only_what_changed() {
        use studio_core::Store as _;
        let (mut host, page, store) = host();
        host.open(&spec()).unwrap();
        assert_eq!(*page.files.lock().unwrap(), store.files());

        assert_eq!(host.reload().unwrap(), Reload::Unchanged);
        store.write("Main.qml", "Rectangle {}\n").unwrap();
        store.remove("Other.qml").unwrap();
        assert_eq!(host.reload().unwrap(), Reload::Applied { generation: 1 });
        assert_eq!(
            page.heard.lock().unwrap().last().unwrap(),
            &Request::Reload {
                changed: [
                    ("Main.qml".to_owned(), Some("Rectangle {}\n".to_owned())),
                    ("Other.qml".to_owned(), None),
                ]
                .into()
            }
        );
        assert_eq!(*page.files.lock().unwrap(), store.files());
        assert_eq!(host.outline().unwrap(), "Main.qml");
    }

    #[test]
    fn answers_become_what_the_host_returns() {
        let (mut host, page, _) = host();
        assert!(matches!(
            host.open(&PreviewSpec {
                entry: "Nope.qml".into(),
                ..spec()
            }),
            Err(PreviewError::Start(_))
        ));
        host.open(&spec()).unwrap();
        let picture = host.screenshot(100).unwrap();
        assert_eq!((picture.png.len(), picture.width), (4, 2));
        assert_eq!(
            host.pick(1.0, 1.0).unwrap(),
            Some(Declared {
                file: "Main.qml".into(),
                line: 3,
                column: 1
            })
        );
        assert_eq!(host.pick(-1.0, 0.0).unwrap(), None);
        host.click(&Click::Id("card".into())).unwrap();
        host.click(&Click::Point { x: 4.0, y: 6.0 }).unwrap();
        assert_eq!(
            page.heard.lock().unwrap().last().unwrap(),
            &Request::Click {
                id: None,
                at: Some([4.0, 6.0])
            }
        );
        assert!(matches!(
            host.click(&Click::Id("nobody".into())),
            Err(PreviewError::NoObject(id)) if id == "nobody"
        ));
        assert!(matches!(
            host.read("nobody", &["x".into()]),
            Err(PreviewError::NoObject(id)) if id == "nobody"
        ));
        assert_eq!(host.logs(0).unwrap().1, 1);
        assert!(matches!(
            host.write("card", "width", json!(1)),
            Err(PreviewError::NotSupported {
                ljos_web_request: 4,
                ..
            })
        ));
        assert!(matches!(
            host.resize(Size {
                width: 1,
                height: 1
            }),
            Ok(())
        ));
    }

    #[test]
    fn a_page_that_is_gone_ends_the_preview() {
        let (mut host, page, _) = host();
        host.open(&spec()).unwrap();
        *page.gone.lock().unwrap() = true;
        assert!(matches!(host.outline(), Err(PreviewError::Ended(_))));
        assert!(matches!(host.outline(), Err(PreviewError::NotOpen)));
    }

    #[test]
    fn a_page_that_is_gone_is_told_without_asking_it() {
        let (mut host, page, _) = host();
        assert!(matches!(host.alive(), Err(PreviewError::NotOpen)));
        host.open(&spec()).unwrap();
        assert!(host.alive().is_ok());
        let asked = page.heard.lock().unwrap().len();
        *page.gone.lock().unwrap() = true;
        assert!(matches!(host.alive(), Err(PreviewError::Ended(_))));
        assert_eq!(page.heard.lock().unwrap().len(), asked);
        assert!(matches!(host.alive(), Err(PreviewError::NotOpen)));
    }

    #[test]
    fn a_call_the_page_cannot_do_yet_leaves_the_preview_alive() {
        let (mut host, _, _) = host();
        host.open(&spec()).unwrap();
        assert!(matches!(
            host.write("card", "width", json!(1)),
            Err(PreviewError::NotSupported { .. })
        ));
        assert!(host.alive().is_ok());
    }
}
