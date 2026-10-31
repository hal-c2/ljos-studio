//! The `wasm-bindgen` glue: [`Face`] for JavaScript, the page's clock and
//! `setTimeout` as the server's [`Timer`], and the page's `ask` as the
//! preview [`Port`].

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

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

    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    fn date_now() -> f64;

    #[wasm_bindgen(js_name = setTimeout)]
    fn set_timeout(then: &JsValue, ms: f64) -> JsValue;
}

/// The page's clocks and its event loop's timeouts.
struct WebTimer;

impl Timer for WebTimer {
    fn now(&self) -> Duration {
        Duration::from_secs_f64(performance_now().max(0.0) / 1000.0)
    }

    fn wall(&self) -> Duration {
        Duration::from_secs_f64(date_now().max(0.0) / 1000.0)
    }

    fn after(&self, delay: Duration, then: Box<dyn FnOnce() + Send>) {
        let then = Closure::once_into_js(then);
        set_timeout(&then, delay.as_secs_f64() * 1000.0);
    }
}

/// Which [`StudioCore`] a JS function belongs to. One wasm instance serves
/// every core a page or runner opens, so what is kept per instance below
/// is kept per core.
type CoreId = u32;

thread_local! {
    /// Each core's `ask(requestJson) -> answerJson`. A wasm instance has one
    /// thread, and a JS function cannot cross threads, so it is kept here
    /// rather than in the host (which must be `Send`).
    static ASK: RefCell<HashMap<CoreId, js_sys::Function>> = RefCell::new(HashMap::new());
    /// What each core's `watch` was given, for the same reason: the
    /// session's watcher must be `Send`, a JS function is not.
    static WATCHERS: RefCell<HashMap<CoreId, Vec<js_sys::Function>>> =
        RefCell::new(HashMap::new());
    static NEXT_CORE: Cell<CoreId> = const { Cell::new(0) };
}

/// Asks the page through its core's [`ASK`]: the function throws when the
/// page is gone or does not answer in time (`web/preview.js`).
struct JsPort(CoreId);

impl Port for JsPort {
    fn ask(&mut self, request: &Request) -> Result<Answer, PortError> {
        let request = serde_json::to_string(request).expect("a request is JSON");
        // Out of the map before the call: the page's function may close a
        // Studio, which changes the map.
        let ask = ASK
            .with_borrow(|asks| asks.get(&self.0).cloned())
            .ok_or_else(|| PortError::Gone("this page has no preview".to_owned()))?;
        let answer = ask
            .call1(&JsValue::NULL, &JsValue::from_str(&request))
            .map_err(|e| {
                let message = js_message(&e);
                if message.starts_with("gone:") {
                    PortError::Gone(message)
                } else {
                    PortError::NoAnswer(message)
                }
            })?;
        let answer = answer
            .as_string()
            .ok_or_else(|| PortError::NoAnswer("the page's answer is not text".to_owned()))?;
        serde_json::from_str(&answer)
            .map_err(|e| PortError::NoAnswer(format!("the page's answer is not one: {e}")))
    }
    fn gone(&self) -> Option<String> {
        let Some(ask) = ASK.with_borrow(|asks| asks.get(&self.0).cloned()) else {
            return Some("this page has no preview".to_owned());
        };
        // `workerAsk`'s function reads the relay's gone word; a page's own
        // function without one is taken at its word until it throws.
        let gone = js_sys::Reflect::get(&ask, &JsValue::from_str("gone"))
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok())?;
        gone.call0(&JsValue::NULL)
            .is_ok_and(|g| g.is_truthy())
            .then(|| "gone: the preview frame was removed".to_owned())
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
    /// `None` once closed. An `Rc` so a call under way keeps its session
    /// when the page's function it is calling closes this core.
    face: RefCell<Option<Rc<Face>>>,
    id: CoreId,
    /// Whether the session already calls this core's watchers.
    watching: Cell<bool>,
}

impl Drop for StudioCore {
    fn drop(&mut self) {
        self.close();
    }
}

/// How many JS functions the cores of this wasm instance still hold: the
/// page's way to see that closing a Studio let go of them (`web/cores.mjs`),
/// which a garbage collector's timing cannot show.
#[wasm_bindgen(js_name = heldFunctions)]
pub fn held_functions() -> usize {
    ASK.with_borrow(HashMap::len)
        + WATCHERS.with_borrow(|all| all.values().map(Vec::len).sum::<usize>())
}

const CLOSED: &str = "this Studio is closed";

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
        let id = NEXT_CORE.replace(NEXT_CORE.get().wrapping_add(1));
        let hosts = ask.as_ref().map(|_| {
            let store = store.clone();
            Arc::new(move || {
                Box::new(WebPreviewHost::new(JsPort(id), store.clone()))
                    as Box<dyn studio_core::preview::PreviewHost>
            }) as studio_mcp::preview::StoreHostFactory
        });
        let face = Face::new(store, caps, Arc::new(WebTimer), hosts).map_err(error)?;
        // Only now: a core that was never made has no `Drop` to take it out.
        if let Some(ask) = ask {
            ASK.with_borrow_mut(|asks| asks.insert(id, ask));
        }
        Ok(StudioCore {
            face: RefCell::new(Some(Rc::new(face))),
            id,
            watching: Cell::new(false),
        })
    }

    /// Lets go of the session, the page's functions and everything they
    /// hold. Nothing else does in a page: `watch` was given a function that
    /// holds the `Studio` that holds this core, so the core stays reachable
    /// and its finalizer never runs. Every later call is refused, and
    /// closing again does nothing.
    pub fn close(&self) {
        drop(self.face.borrow_mut().take());
        ASK.with_borrow_mut(|asks| asks.remove(&self.id));
        WATCHERS.with_borrow_mut(|all| all.remove(&self.id));
    }

    fn face(&self) -> Result<Rc<Face>, JsError> {
        self.face
            .borrow()
            .clone()
            .ok_or_else(|| JsError::new(CLOSED))
    }

    /// A tool's answer or refusal, as JSON (see [`Face::call`]); `client`
    /// is the caller's name, left out for one that gave none.
    pub fn call(&self, tool: &str, args: &str, client: Option<String>) -> String {
        let Some(face) = self.face.borrow().clone() else {
            return serde_json::json!({"error": {
                "code": "unavailable",
                "message": CLOSED,
                "details": {"kind": "closed"},
            }})
            .to_string();
        };
        let out = match serde_json::from_str(args) {
            Ok(args) => face.call_as(tool, args, client),
            Err(e) => serde_json::json!({"error": {
                "code": "invalid_argument",
                "message": format!("the arguments are not JSON: {e}"),
            }}),
        };
        out.to_string()
    }

    pub fn tools(&self) -> Result<String, JsError> {
        Ok(self.face()?.tools().to_string())
    }

    /// `[{"path", "text"}]`: what storage is to do now (`text: null`
    /// removes). Nothing, once closed.
    pub fn take(&self) -> String {
        let pending = self.face.borrow().as_ref().map(|face| face.take());
        serde_json::to_string(&pending.unwrap_or_default()).expect("pending is JSON")
    }

    /// Storage did not do these (`take`'s JSON): they are done again with
    /// the next. Once closed there is no next, and the call that wrote them
    /// is told by its own refusal.
    pub fn unstored(&self, pending: &str) -> Result<(), JsError> {
        let pending: Vec<Pending> = serde_json::from_str(pending).map_err(error)?;
        if let Some(face) = self.face.borrow().as_ref() {
            face.unstored(pending);
        }
        Ok(())
    }

    /// Storage has this (one of `take`'s records) from somewhere other than
    /// this session: reads see it, and it is not stored again.
    pub fn stored(&self, pending: &str) -> Result<(), JsError> {
        let pending: Pending = serde_json::from_str(pending).map_err(error)?;
        self.face()?.stored(pending).map_err(error)
    }

    /// What a window showing `doc` sees of the session, as JSON (see
    /// [`Face::view`]).
    pub fn view(&self, doc: &str) -> Result<String, JsError> {
        Ok(self.face()?.view(doc).to_string())
    }

    /// Calls `watcher` after every write to the session. It is called while
    /// the session is held: it must only note that the view is stale and
    /// read it later (`web/studio.js` does so in a microtask).
    pub fn watch(&self, watcher: js_sys::Function) -> Result<(), JsError> {
        let face = self.face()?;
        let id = self.id;
        WATCHERS.with_borrow_mut(|all| all.entry(id).or_default().push(watcher));
        if !self.watching.replace(true) {
            face.watch(Arc::new(move || {
                let all = WATCHERS.with_borrow(|all| all.get(&id).cloned().unwrap_or_default());
                for watcher in all {
                    let _ = watcher.call0(&JsValue::NULL);
                }
            }));
        }
        Ok(())
    }

    /// `{"text", "generation"}` of a document.
    pub fn text(&self, doc: &str) -> Result<String, JsError> {
        let (text, generation) = self.face()?.text(doc).map_err(error)?;
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
        self.face()?
            .replace_text(doc, generation as u64, start as usize, end as usize, text)
            .map_err(error)
    }
}
