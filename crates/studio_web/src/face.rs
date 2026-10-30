//! What a page calls: Studio's MCP tools over a project in the browser's
//! storage, as JSON in and out.
//!
//! This is the whole of the JavaScript face but the `wasm-bindgen` glue
//! (`wasm.rs`), so it is tested natively. `web/studio.js` wraps it in
//! promises that resolve once storage has what a call wrote.

use std::sync::Arc;

use base64::Engine as _;
use ljos_mcp_core::{CallCtx, CapSet, McpSurface, SurfaceError};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer, preview::StoreHostFactory, timer::Timer};

use crate::{Pending, WebStore};

/// Why the page's own calls (not tools) are refused.
#[derive(Debug, thiserror::Error)]
pub enum FaceError {
    #[error("the capabilities are not a list of capabilities: {0}")]
    Caps(String),
    #[error("{0}")]
    Session(#[from] studio_core::Error),
    #[error("{0} is not a place in the text: it is past its end or inside a character")]
    Offset(usize),
}

pub struct Face {
    server: StudioServer,
    store: WebStore,
    caps: CapSet,
}

impl Face {
    /// A Studio over `store`, its tools offered as `caps` allows (a list as
    /// `--caps` takes it: `inspect,mutate`), previews made by `hosts`.
    pub fn new(
        store: WebStore,
        caps: &str,
        timer: Arc<dyn Timer>,
        hosts: Option<StoreHostFactory>,
    ) -> Result<Self, FaceError> {
        let caps = CapSet::parse_list(caps).map_err(|e| FaceError::Caps(e.to_string()))?;
        let mut studio = Studio::with_store(Box::new(store.clone()), timer);
        if let Some(hosts) = hosts {
            studio = studio.with_preview_hosts(hosts);
        }
        Ok(Self {
            server: StudioServer::new(studio),
            store,
            caps,
        })
    }

    pub fn store(&self) -> &WebStore {
        &self.store
    }

    /// Runs `tool` as a client's call does: `{"ok": …, "text": …,
    /// "images": [{"mime", "data"}]}` with each image's bytes as base64, or
    /// the refusal, `{"error": {"code", "message", …}}`.
    pub fn call(&self, tool: &str, args: Value) -> Value {
        match self.server.call(tool, &CallCtx::new(self.caps), args) {
            Ok(reply) => json!({
                "ok": reply.structured,
                "text": reply.text,
                "images": reply.images.iter().map(|i| json!({
                    "mime": i.mime,
                    "data": base64::engine::general_purpose::STANDARD.encode(&i.data),
                })).collect::<Vec<_>>(),
            }),
            Err(e) => refusal(&e),
        }
    }

    /// The tools this page's capabilities offer, as `tools/list` gives
    /// them.
    pub fn tools(&self) -> Value {
        self.server
            .tools()
            .visible(self.caps)
            .map(|d| {
                json!({
                    "name": d.name,
                    "description": d.description,
                    "inputSchema": *d.input_schema,
                })
            })
            .collect()
    }

    /// What storage has yet to do (see [`WebStore::take`]).
    pub fn take(&self) -> Vec<Pending> {
        self.store.take()
    }

    pub fn unstored(&self, pending: Vec<Pending>) {
        for p in pending {
            self.store.unstored(p);
        }
    }

    /// What storage got from elsewhere (see [`WebStore::stored`]).
    pub fn stored(&self, pending: Pending) -> Result<(), studio_core::StoreError> {
        self.store.stored(pending)
    }

    /// A document's text and generation, as a code view shows it.
    pub fn text(&self, doc: &str) -> Result<(String, u64), FaceError> {
        let studio = self.server.studio();
        let mut studio = studio_lock(&studio);
        let (text, generation) = studio.session.text(doc)?;
        Ok((text.to_owned(), generation))
    }

    /// The session as a window showing `doc` sees it, as JSON: what
    /// `Studio.Workspace` publishes natively (`studio_mcp::view`), so a
    /// face of it on the web says the same.
    pub fn view(&self, doc: &str) -> Value {
        let studio = self.server.studio();
        let view = studio_lock(&studio).view(doc);
        serde_json::to_value(view).unwrap_or_default()
    }

    /// Calls `watcher` after every write to the session, whoever made it:
    /// a preview that reloaded on a timer too, not only a call. It may run
    /// while the session is held, so it must not call back in; it should
    /// only note that the view is stale.
    pub fn watch(&self, watcher: studio_mcp::Watcher) {
        studio_lock(&self.server.studio()).watch(watcher);
    }

    /// A person's typing in a code view: `start..end` of the text at
    /// `generation` becomes `text`. Offsets count UTF-16 units, as a
    /// JavaScript string does; the session counts bytes.
    pub fn replace_text(
        &self,
        doc: &str,
        generation: u64,
        start: usize,
        end: usize,
        text: &str,
    ) -> Result<(), FaceError> {
        let studio = self.server.studio();
        let mut studio = studio_lock(&studio);
        let (current, _) = studio.session.text(doc)?;
        let range = byte_offset(current, start)?..byte_offset(current, end)?;
        studio
            .session
            .replace_text(doc, Some(generation), Some(range), text, false)?;
        Ok(())
    }
}

fn studio_lock(studio: &std::sync::Mutex<Studio>) -> std::sync::MutexGuard<'_, Studio> {
    studio
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn refusal(e: &SurfaceError) -> Value {
    e.to_value()
}

/// The byte offset of the UTF-16 offset `units` in `text`.
fn byte_offset(text: &str, units: usize) -> Result<usize, FaceError> {
    let mut counted = 0;
    for (byte, c) in text.char_indices() {
        if counted == units {
            return Ok(byte);
        }
        if counted > units {
            break;
        }
        counted += c.len_utf16();
    }
    if counted == units {
        Ok(text.len())
    } else {
        Err(FaceError::Offset(units))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    struct NoTime;
    impl Timer for NoTime {
        fn now(&self) -> Duration {
            Duration::ZERO
        }
        fn after(&self, _: Duration, _: Box<dyn FnOnce() + Send>) {}
    }

    fn face(caps: &str) -> Face {
        let store = WebStore::new(
            "memory",
            [(
                "Main.qml".to_owned(),
                "import QtQuick\n\nItem {\n    id: root\n    width: 10\n}\n".to_owned(),
            )],
        );
        Face::new(store, caps, Arc::new(NoTime), None).unwrap()
    }

    #[test]
    fn a_tool_answers_and_what_it_wrote_waits_for_storage() {
        let face = face("inspect,mutate");
        let tree = face.call("document_tree", json!({"doc": "Main.qml"}));
        assert!(tree["ok"]["nodes"].is_array(), "{tree}");
        let set = face.call(
            "property_set",
            json!({"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 20}),
        );
        assert!(set.get("ok").is_some(), "{set}");
        let pending = face.take();
        assert_eq!(pending.len(), 1);
        assert!(pending[0].text.as_deref().unwrap().contains("width: 20"));
    }

    #[test]
    fn a_refusal_is_the_servers_refusal() {
        let face = face("inspect");
        let out = face.call("property_set", json!({}));
        assert!(out["error"]["code"].is_string(), "{out}");
        assert!(
            face.tools()
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["name"] != "property_set")
        );
    }

    #[test]
    fn typing_counts_as_javascript_counts() {
        let face = face("inspect,mutate");
        let store = face.store().clone();
        use studio_core::Store as _;
        store
            .write("Emoji.qml", "Text { text: \"😀x\" }\n")
            .unwrap();
        let (_, generation) = face.text("Emoji.qml").unwrap();
        // `😀` is two units in JavaScript, four bytes here: `x` is at 16.
        face.replace_text("Emoji.qml", generation, 16, 17, "y")
            .unwrap();
        assert_eq!(
            face.text("Emoji.qml").unwrap().0,
            "Text { text: \"😀y\" }\n"
        );
        assert!(matches!(
            face.replace_text("Emoji.qml", generation + 1, 15, 15, ""),
            Err(FaceError::Offset(15))
        ));
    }

    /// What `Workspace.js` publishes: the native block's view, and a
    /// watcher told of a write whoever made it.
    #[test]
    fn the_view_is_the_native_blocks_and_writes_are_told() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let face = face("inspect,mutate");
        let told = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&told);
        face.watch(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        let view = face.view("Main.qml");
        assert_eq!(view["documents"], json!(["Main.qml"]), "{view}");
        assert_eq!(view["layers"][0]["label"], "Item root", "{view}");
        assert_eq!(view["preview"], Value::Null);

        let set = face.call(
            "property_set",
            json!({"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 20}),
        );
        assert!(set.get("ok").is_some(), "{set}");
        assert!(told.load(Ordering::SeqCst) > 0, "the write was told");
        assert!(
            face.view("Main.qml")["text"]
                .as_str()
                .unwrap()
                .contains("width: 20")
        );
    }

    /// A burst of writes within one second reloads a page once a display
    /// frame at 60 Hz (`host::CALLS_PER_SECOND`); the rest wait for a
    /// reading or the next second.
    #[test]
    fn a_burst_reloads_a_page_at_most_once_a_frame() {
        use crate::host::{WebPreviewHost, tests::FakePage};
        use crate::protocol::Request;

        let store = WebStore::new(
            "memory",
            [(
                "Main.qml".to_owned(),
                "import QtQuick\n\nItem {\n    id: card\n    width: 10\n}\n".to_owned(),
            )],
        );
        let page = FakePage::default();
        let hosts: StoreHostFactory = {
            let (page, store) = (page.clone(), store.clone());
            Arc::new(move || Box::new(WebPreviewHost::new(page.clone(), store.clone())))
        };
        let face = Face::new(store, "inspect,mutate,run", Arc::new(NoTime), Some(hosts)).unwrap();
        let open = face.call("preview_open", json!({"doc": "Main.qml"}));
        assert!(open.get("ok").is_some(), "{open}");
        for width in 0..100 {
            let set = face.call(
                "property_set",
                json!({"doc": "Main.qml", "node": {"id": "card"}, "name": "width", "value": width}),
            );
            assert!(set.get("ok").is_some(), "{set}");
        }
        let reloads = page
            .heard
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, Request::Reload { .. }))
            .count();
        assert_eq!(reloads, 60);
    }

    /// The preview tools against a page that keeps what it is sent: the
    /// Studio side of the web preview, end to end but for the browser.
    #[test]
    fn the_preview_tools_drive_a_page() {
        use crate::host::{WebPreviewHost, tests::FakePage};
        use crate::protocol::Request;

        let store = WebStore::new(
            "memory",
            [(
                "Main.qml".to_owned(),
                "import QtQuick\n\nItem {\n    id: card\n    width: 10\n}\n".to_owned(),
            )],
        );
        let page = FakePage::default();
        let hosts: StoreHostFactory = {
            let (page, store) = (page.clone(), store.clone());
            Arc::new(move || Box::new(WebPreviewHost::new(page.clone(), store.clone())))
        };
        let face = Face::new(store, "inspect,mutate,run", Arc::new(NoTime), Some(hosts)).unwrap();

        let open = face.call(
            "preview_open",
            json!({"doc": "Main.qml", "target": "phone"}),
        );
        assert_eq!(open["ok"]["size"]["width"], 390, "{open}");
        assert!(page.files.lock().unwrap()["Main.qml"].contains("width: 10"));

        let set = face.call(
            "property_set",
            json!({"doc": "Main.qml", "node": {"id": "card"}, "name": "width", "value": 20}),
        );
        assert!(set.get("ok").is_some(), "{set}");
        assert!(page.files.lock().unwrap()["Main.qml"].contains("width: 20"));
        let events = face.call("preview_events", json!({"since": 1}));
        assert_eq!(events["ok"]["events"][0]["kind"], "reloaded", "{events}");

        let shot = face.call("preview_screenshot", json!({"format": "outline"}));
        assert_eq!(shot["ok"]["outline"], "Main.qml", "{shot}");

        // The fake page says `Main.qml` line 3 column 1 is there; the
        // session finds the object declared there.
        let picked = face.call(
            "node_select",
            json!({"doc": "Main.qml", "at": {"x": 5, "y": 5}}),
        );
        assert_eq!(picked["ok"]["selection"][0]["id"], "card", "{picked}");
        assert!(
            page.heard
                .lock()
                .unwrap()
                .iter()
                .any(|r| matches!(r, Request::Pick { .. })),
            "the page was asked what is there"
        );
    }
}
