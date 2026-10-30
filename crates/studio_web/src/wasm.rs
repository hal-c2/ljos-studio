//! The `wasm-bindgen` glue: [`Face`] for JavaScript, the page's clock and
//! `setTimeout` as the server's [`Timer`], and the page's `ask` as the
//! preview [`Port`].

use std::{cell::RefCell, sync::Arc, time::Duration};

use studio_mcp::timer::Timer;
use wasm_bindgen::{closure::Closure, prelude::*};

use crate::{
    Face, Pending, Port, PortError, WebPreviewHost,
    protocol::{Answer, Request},
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;

    #[wasm_bindgen(js_name = setTimeout)]
    fn set_timeout(then: &JsValue, ms: f64) -> JsValue;
}

/// The page's monotonic clock and its event loop's timeouts.
struct WebTimer;

impl Timer for WebTimer {
    fn now(&self) -> Duration {
        Duration::from_secs_f64(performance_now().max(0.0) / 1000.0)
    }

    fn after(&self, delay: Duration, then: Box<dyn FnOnce() + Send>) {
        let then = Closure::once_into_js(then);
        set_timeout(&then, delay.as_secs_f64() * 1000.0);
    }
}

thread_local! {
    /// The page's `ask(requestJson) -> answerJson`. A wasm instance has one
    /// thread, and a JS function cannot cross threads, so it is kept here
    /// rather than in the host (which must be `Send`).
    static ASK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    /// What `watch` was given, for the same reason: the session's watcher
    /// must be `Send`, a JS function is not.
    static WATCHERS: RefCell<Vec<js_sys::Function>> = const { RefCell::new(Vec::new()) };
}

/// Asks the page through [`ASK`]: the function throws when the page is
/// gone or does not answer in time (`web/preview.js`).
struct JsPort;

impl Port for JsPort {
    fn ask(&mut self, request: &Request) -> Result<Answer, PortError> {
        let request = serde_json::to_string(request).expect("a request is JSON");
        let answer = ASK.with_borrow(|ask| {
            let ask = ask
                .as_ref()
                .ok_or_else(|| PortError::Gone("this page has no preview".to_owned()))?;
            ask.call1(&JsValue::NULL, &JsValue::from_str(&request))
                .map_err(|e| {
                    let message = js_message(&e);
                    if message.starts_with("gone:") {
                        PortError::Gone(message)
                    } else {
                        PortError::NoAnswer(message)
                    }
                })
        })?;
        let answer = answer
            .as_string()
            .ok_or_else(|| PortError::NoAnswer("the page's answer is not text".to_owned()))?;
        serde_json::from_str(&answer)
            .map_err(|e| PortError::NoAnswer(format!("the page's answer is not one: {e}")))
    }
}

fn js_message(e: &JsValue) -> String {
    e.as_string()
        .or_else(|| {
            js_sys::Reflect::get(e, &JsValue::from_str("message"))
                .ok()
                .and_then(|m| m.as_string())
        })
        .unwrap_or_else(|| "the page threw".to_owned())
}

fn error(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Studio's session and tools, for `web/studio.js`. Everything crosses as
/// JSON text: one parse per call either side is cheaper than walking
/// objects through the glue.
#[wasm_bindgen]
pub struct StudioCore {
    face: Face,
}

#[wasm_bindgen]
impl StudioCore {
    /// `files` is `{"path": "text", …}` as storage has them; `caps` a list
    /// such as `inspect,mutate`; `ask`, when given, reaches a preview page.
    #[wasm_bindgen(constructor)]
    pub fn new(
        place: &str,
        files: &str,
        caps: &str,
        ask: Option<js_sys::Function>,
    ) -> Result<StudioCore, JsError> {
        let files: std::collections::BTreeMap<String, String> =
            serde_json::from_str(files).map_err(error)?;
        let store = crate::WebStore::new(place, files);
        let hosts = ask.map(|ask| {
            ASK.with_borrow_mut(|slot| *slot = Some(ask));
            let store = store.clone();
            Arc::new(move || {
                Box::new(WebPreviewHost::new(JsPort, store.clone()))
                    as Box<dyn studio_core::preview::PreviewHost>
            }) as studio_mcp::preview::StoreHostFactory
        });
        let face = Face::new(store, caps, Arc::new(WebTimer), hosts).map_err(error)?;
        Ok(StudioCore { face })
    }

    /// A tool's answer or refusal, as JSON (see [`Face::call`]).
    pub fn call(&self, tool: &str, args: &str) -> String {
        let out = match serde_json::from_str(args) {
            Ok(args) => self.face.call(tool, args),
            Err(e) => serde_json::json!({"error": {
                "code": "invalid_argument",
                "message": format!("the arguments are not JSON: {e}"),
            }}),
        };
        out.to_string()
    }

    pub fn tools(&self) -> String {
        self.face.tools().to_string()
    }

    /// `[{"path", "text"}]`: what storage is to do now (`text: null`
    /// removes).
    pub fn take(&self) -> String {
        serde_json::to_string(&self.face.take()).expect("pending is JSON")
    }

    /// Storage did not do these (`take`'s JSON): they are done again with
    /// the next.
    pub fn unstored(&self, pending: &str) -> Result<(), JsError> {
        let pending: Vec<Pending> = serde_json::from_str(pending).map_err(error)?;
        self.face.unstored(pending);
        Ok(())
    }

    /// Storage has this (one of `take`'s records) from somewhere other than
    /// this session: reads see it, and it is not stored again.
    pub fn stored(&self, pending: &str) -> Result<(), JsError> {
        let pending: Pending = serde_json::from_str(pending).map_err(error)?;
        self.face.stored(pending).map_err(error)
    }

    /// What a window showing `doc` sees of the session, as JSON (see
    /// [`Face::view`]).
    pub fn view(&self, doc: &str) -> String {
        self.face.view(doc).to_string()
    }

    /// Calls `watcher` after every write to the session. It is called while
    /// the session is held: it must only note that the view is stale and
    /// read it later (`web/studio.js` does so in a microtask).
    pub fn watch(&self, watcher: js_sys::Function) {
        let first = WATCHERS.with_borrow_mut(|all| {
            all.push(watcher);
            all.len() == 1
        });
        if first {
            self.face.watch(Arc::new(|| {
                let all = WATCHERS.with_borrow(Clone::clone);
                for watcher in all {
                    let _ = watcher.call0(&JsValue::NULL);
                }
            }));
        }
    }

    /// `{"text", "generation"}` of a document.
    pub fn text(&self, doc: &str) -> Result<String, JsError> {
        let (text, generation) = self.face.text(doc).map_err(error)?;
        Ok(serde_json::json!({"text": text, "generation": generation}).to_string())
    }

    /// A code view's edit, in UTF-16 offsets as the string it showed.
    pub fn replace_text(
        &self,
        doc: &str,
        generation: f64,
        start: u32,
        end: u32,
        text: &str,
    ) -> Result<(), JsError> {
        self.face
            .replace_text(doc, generation as u64, start as usize, end as usize, text)
            .map_err(error)
    }
}
