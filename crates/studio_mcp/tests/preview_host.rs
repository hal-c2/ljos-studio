//! What the preview tools ask of their host, with a host that records the
//! asking: the scenarios (`features/preview`) show the same through a real
//! `ljos`, and these pin the calls in between without one.

use std::sync::{Arc, Mutex};

use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::{Map, Value, json};
use studio_core::preview::{
    Declared, LogLine, Picture, PreviewError, PreviewHost, PreviewSpec, Reload, Size,
};
use studio_mcp::{Studio, StudioServer};

#[derive(Default)]
struct Seen {
    calls: Vec<String>,
    generation: u64,
    fail_next: bool,
    /// Calls that answer with an error, by how the call's note starts; each
    /// answers once.
    errors: Vec<(String, PreviewError)>,
    /// What `pick` answers.
    declared: Option<Declared>,
}

struct Recorder(Arc<Mutex<Seen>>);

impl Recorder {
    fn note(&self, call: String) -> Result<(), PreviewError> {
        let answer = self.answer(&call);
        self.0.lock().unwrap().calls.push(call);
        answer
    }

    /// The error a call was set up to answer with, if one was.
    fn answer(&self, call: &str) -> Result<(), PreviewError> {
        let mut seen = self.0.lock().unwrap();
        match seen
            .errors
            .iter()
            .position(|(start, _)| call.starts_with(start))
        {
            Some(at) => Err(seen.errors.remove(at).1),
            None => Ok(()),
        }
    }
}

impl PreviewHost for Recorder {
    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError> {
        let aliases: Vec<String> = spec
            .aliases
            .iter()
            .map(|a| format!("{}={}", a.module, a.dir))
            .collect();
        self.note(format!(
            "open {} {}x{} [{}]",
            spec.entry,
            spec.size.width,
            spec.size.height,
            aliases.join(",")
        ))?;
        Ok(())
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        self.note("reload".to_owned())?;
        let mut seen = self.0.lock().unwrap();
        if std::mem::take(&mut seen.fail_next) {
            return Ok(Reload::Failed {
                message: "Shell.qml:4: expected a property".to_owned(),
            });
        }
        seen.generation += 1;
        Ok(Reload::Applied {
            generation: seen.generation,
        })
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        self.note(format!("alias {module} {}", dir.unwrap_or("-")))?;
        let mut seen = self.0.lock().unwrap();
        seen.generation += 1;
        Ok(Reload::Applied {
            generation: seen.generation,
        })
    }

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        self.note(format!("resize {}x{}", size.width, size.height))?;
        Ok(())
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        self.note(format!("screenshot {max_side}"))?;
        Ok(Picture {
            png: b"\x89PNG".to_vec(),
            width: max_side,
            height: max_side / 2,
        })
    }

    fn outline(&mut self) -> Result<String, PreviewError> {
        self.answer("outline")?;
        Ok("text \"Stopped\"".to_owned())
    }

    fn read(&mut self, id: &str, names: &[String]) -> Result<Map<String, Value>, PreviewError> {
        self.note(format!("read {id} {}", names.join(",")))?;
        Ok(names
            .iter()
            .map(|n| (n.clone(), Value::Bool(false)))
            .collect())
    }

    fn write(&mut self, id: &str, name: &str, value: Value) -> Result<(), PreviewError> {
        self.note(format!("write {id}.{name}={value}"))?;
        Ok(())
    }

    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
        self.answer("logs")?;
        let lines = if since == 0 {
            vec![LogLine {
                level: "warning".to_owned(),
                message: "a warning".to_owned(),
                file: None,
                line: None,
            }]
        } else {
            Vec::new()
        };
        Ok((lines, 1))
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        self.answer("alive")
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        self.note(format!("pick {x},{y}"))?;
        Ok(self.0.lock().unwrap().declared.clone())
    }

    fn close(&mut self) {
        let _ = self.note("close".to_owned());
    }
}

const SHELL: &str = "import QtQuick\nimport App.Media\n\nColumn {\n    Player { id: player1 }\n    Text { text: \"x\" }\n}\n";

struct Fixture {
    _dir: tempfile::TempDir,
    server: StudioServer,
    seen: Arc<Mutex<Seen>>,
    opened: Arc<Mutex<usize>>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let files = [
            (
                "App/Media/qmldir",
                "module App.Media\n# block Player 1.0 Player.qml\n",
            ),
            (
                "App/Media/Player.qml",
                "import QtQuick\n\nItem {\n    readonly property bool playing: false\n}\n",
            ),
            (
                "mocks/App/Media/qmldir",
                "module App.Media\nPlayer 1.0 Player.qml\n",
            ),
            (
                "mocks/App/Media/Player.qml",
                "import QtQuick\n\nItem {\n    property bool playing: true\n}\n",
            ),
            ("Shell.qml", SHELL),
            ("Other.qml", "import QtQuick\n\nItem {}\n"),
        ];
        for (name, text) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let seen = Arc::new(Mutex::new(Seen::default()));
        let opened = Arc::new(Mutex::new(0));
        let studio = Studio::open(dir.path()).unwrap().with_previews(Arc::new({
            let seen = seen.clone();
            let opened = opened.clone();
            move |_| {
                *opened.lock().unwrap() += 1;
                Box::new(Recorder(seen.clone()))
            }
        }));
        Fixture {
            _dir: dir,
            server: StudioServer::new(studio),
            seen,
            opened,
        }
    }

    fn call(&self, tool: &str, args: Value) -> Result<Value, Value> {
        let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run]));
        self.server
            .call(tool, &ctx, args)
            .map(|r| r.structured)
            .map_err(|e| e.to_value()["error"].clone())
    }

    fn fail(&self, call: &str, error: PreviewError) {
        self.seen
            .lock()
            .unwrap()
            .errors
            .push((call.to_owned(), error));
    }

    fn events(&self) -> Vec<Value> {
        let out = self.call("preview_events", json!({})).unwrap();
        out["events"].as_array().unwrap().clone()
    }

    fn calls(&self) -> Vec<String> {
        std::mem::take(&mut self.seen.lock().unwrap().calls)
    }
}

#[test]
fn a_preview_opens_on_a_target_with_its_mocks() {
    let f = Fixture::new();
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "target": "tablet", "state": "mocked" }),
        )
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 820, "height": 1180 }));
    assert_eq!(out["mocked"], json!(["App.Media"]));
    assert_eq!(out["started"], true);
    assert_eq!(
        f.calls(),
        ["open Shell.qml 820x1180 [App.Media=mocks/App/Media]"]
    );
}

#[test]
fn reopening_changes_the_open_preview_in_place() {
    let f = Fixture::new();
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "state": "mocked" }),
    )
    .unwrap();
    f.calls();
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "target": "phone", "state": "live" }),
        )
        .unwrap();
    assert_eq!(out["started"], false);
    assert_eq!(f.calls(), ["resize 390x844", "alias App.Media -"]);
    // Back to mocked keeps the phone's size.
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "state": "mocked" }),
        )
        .unwrap();
    assert_eq!(out["target"], "phone");
    assert_eq!(f.calls(), ["alias App.Media mocks/App/Media"]);
    assert_eq!(*f.opened.lock().unwrap(), 1);
}

#[test]
fn another_document_starts_another_preview() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.call("preview_open", json!({ "doc": "Other.qml" }))
        .unwrap();
    assert_eq!(*f.opened.lock().unwrap(), 2);
    assert_eq!(
        f.calls(),
        [
            "open Shell.qml 1280x800 []",
            "close",
            "open Other.qml 1280x800 []"
        ]
    );
}

#[test]
fn every_write_reloads_the_preview_before_the_tool_answers() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    f.call(
        "binding_set",
        json!({ "doc": "Shell.qml", "generation": 1, "node": { "path": "root/1" }, "name": "text", "value": "\"Ready\"" }),
    )
    .unwrap();
    assert_eq!(f.calls(), ["reload"]);
    f.seen.lock().unwrap().fail_next = true;
    f.call("history", json!({ "action": "undo" })).unwrap();
    assert_eq!(f.calls(), ["reload"]);
    let events = f.call("preview_events", json!({ "since": 1 })).unwrap();
    let kinds: Vec<&str> = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["reloaded", "reload_failed", "log"]);
    assert_eq!(events["next"], 4);
    let rest = f.call("preview_events", json!({ "since": 4 })).unwrap();
    assert_eq!(rest["events"], json!([]));
}

/// A preview's inspector answers a bounded number of calls a second; a
/// burst of writes (typing in the code view) must not wait out its limit.
/// What a burst leaves unreloaded the preview's own watcher shows, and any
/// reading of the preview catches up first.
#[test]
fn a_burst_of_writes_reloads_within_the_budget_and_a_reading_catches_up() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    for i in 0..20 {
        f.call(
            "binding_set",
            json!({ "doc": "Shell.qml", "generation": i + 1, "node": { "path": "root/1" }, "name": "text", "value": format!("\"{i}\"") }),
        )
        .unwrap();
    }
    let reloads = f.calls().len();
    assert!(
        (1..20).contains(&reloads),
        "{reloads} reloads for 20 writes in a burst"
    );
    f.call("preview_screenshot", json!({})).unwrap();
    assert_eq!(f.calls(), ["reload", "screenshot 800"]);
    f.call("preview_screenshot", json!({})).unwrap();
    assert_eq!(f.calls(), ["screenshot 800"]);
}

#[test]
fn a_live_block_is_read_but_not_set() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    let out = f
        .call("preview_state", json!({ "node": { "id": "player1" } }))
        .unwrap();
    assert_eq!(out["block"], "App.Media.Player");
    assert_eq!(out["state"], json!({ "playing": false }));
    let refused = f
        .call(
            "preview_state",
            json!({ "node": { "id": "player1" }, "set": { "playing": true } }),
        )
        .unwrap_err();
    assert_eq!(refused["code"], "not_actionable");
    assert_eq!(f.calls(), ["read player1 playing"]);
}

#[test]
fn a_mocked_block_is_set_by_its_state_names_only() {
    let f = Fixture::new();
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "state": "mocked" }),
    )
    .unwrap();
    f.calls();
    let refused = f
        .call(
            "preview_state",
            json!({ "node": { "id": "player1" }, "set": { "volume": 3 } }),
        )
        .unwrap_err();
    assert_eq!(refused["code"], "invalid_argument");
    f.call(
        "preview_state",
        json!({ "node": { "id": "player1" }, "set": { "playing": false } }),
    )
    .unwrap();
    assert_eq!(
        f.calls(),
        ["write player1.playing=false", "read player1 playing"]
    );
}

#[test]
fn an_object_that_is_not_a_block_has_no_state() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let refused = f
        .call("preview_state", json!({ "node": { "path": "root/1" } }))
        .unwrap_err();
    assert_eq!(refused["code"], "invalid_argument");
}

#[test]
fn a_screenshot_is_a_picture_and_its_size() {
    let f = Fixture::new();
    let ctx = CallCtx::new(CapSet::of([Cap::Inspect]));
    let none = f
        .server
        .call("preview_screenshot", &ctx, json!({}))
        .unwrap_err();
    assert_eq!(none.to_value()["error"]["code"], "not_actionable");
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let reply = f
        .server
        .call("preview_screenshot", &ctx, json!({ "max_size": 5000 }))
        .unwrap();
    assert_eq!(reply.structured["width"], 1280);
    assert_eq!(reply.images.len(), 1);
    assert_eq!(reply.images[0].mime, "image/png");
    let outline = f
        .server
        .call("preview_screenshot", &ctx, json!({ "format": "outline" }))
        .unwrap();
    assert_eq!(outline.structured["outline"], "text \"Stopped\"");
    assert!(outline.images.is_empty());
}

#[test]
fn a_reader_may_look_but_not_start_a_preview() {
    let f = Fixture::new();
    let ctx = CallCtx::new(CapSet::of([Cap::Inspect]));
    let refused = f
        .server
        .call("preview_open", &ctx, json!({ "doc": "Shell.qml" }))
        .unwrap_err();
    assert_eq!(refused.to_value()["error"]["code"], "capability_denied");
}

#[test]
fn a_project_without_mocks_cannot_preview_mocked() {
    let f = Fixture::new();
    std::fs::remove_dir_all(f._dir.path().join("mocks")).unwrap();
    let refused = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "state": "mocked" }),
        )
        .unwrap_err();
    assert_eq!(refused["details"]["kind"], "no_mocks");
}

fn event_kinds(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(|e| e["kind"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn ended() -> PreviewError {
    PreviewError::Ended("the process exited".to_owned())
}

fn refused() -> PreviewError {
    PreviewError::Refused {
        code: "rate_limited".to_owned(),
        message: "too many calls".to_owned(),
    }
}

/// Review 4427296674: every host call that says the preview is over must end
/// it here, or the next `preview_open` reuses a process that is gone.
#[test]
fn a_call_that_finds_the_preview_ended_ends_it_and_the_next_open_starts_one() {
    for (call, args) in [
        ("screenshot", json!({})),
        ("outline", json!({ "format": "outline" })),
    ] {
        let f = Fixture::new();
        f.call("preview_open", json!({ "doc": "Shell.qml" }))
            .unwrap();
        f.fail(call, ended());
        assert_eq!(
            f.call("preview_screenshot", args).unwrap_err()["details"]["kind"],
            "ended"
        );
        f.calls();
        let out = f
            .call("preview_open", json!({ "doc": "Shell.qml" }))
            .unwrap();
        assert_eq!(out["started"], true, "{call}");
        assert_eq!(f.calls()[0], "open Shell.qml 1280x800 []", "{call}");
        assert_eq!(*f.opened.lock().unwrap(), 2, "{call}");
    }
}

/// Review 4429187143: a preview whose process ended while nothing read it
/// is found ended by the reopen itself, which starts a new one rather than
/// answering for a process that is gone.
#[test]
fn a_reopen_that_finds_the_preview_ended_starts_one() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    f.fail("alive", ended());
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["started"], true);
    assert_eq!(f.calls(), ["open Shell.qml 1280x800 []"]);
    assert_eq!(*f.opened.lock().unwrap(), 2);
    assert_eq!(event_kinds(&f.events())[..3], ["opened", "ended", "opened"]);
}

#[test]
fn a_block_read_or_a_resize_that_finds_the_preview_ended_ends_it() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.fail("read", ended());
    f.call("preview_state", json!({ "node": { "id": "player1" } }))
        .unwrap_err();
    assert!(event_kinds(&f.events()).contains(&"ended".to_owned()));
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(*f.opened.lock().unwrap(), 2);

    f.fail("resize", ended());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "target": "phone" }),
    )
    .unwrap_err();
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["started"], true);
    assert_eq!(*f.opened.lock().unwrap(), 3);
}

#[test]
fn a_refused_screenshot_leaves_the_preview_open() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.fail("screenshot", refused());
    f.call("preview_screenshot", json!({})).unwrap_err();
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["started"], false);
    assert_eq!(*f.opened.lock().unwrap(), 1);
}

/// A project with three mock modules, for failures part way through them.
fn three_mocks(f: &Fixture) {
    for module in ["A", "B", "C"] {
        let path = f._dir.path().join("mocks").join(module);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("qmldir"), format!("module {module}\n")).unwrap();
    }
    std::fs::remove_dir_all(f._dir.path().join("mocks/App")).unwrap();
}

/// Review 4427296683.
#[test]
fn a_failure_on_the_second_of_three_aliases_keeps_the_record_true() {
    let f = Fixture::new();
    three_mocks(&f);
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    let before = f.events().len();

    f.fail("alias B", refused());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "state": "mocked" }),
    )
    .unwrap_err();
    assert_eq!(f.calls(), ["alias A mocks/A", "alias B mocks/B"]);
    // The reload the first alias caused is on record.
    let events = f.events();
    assert_eq!(event_kinds(&events[before..])[0], "reloaded");

    // A retry does only what the host has not done.
    f.calls();
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "state": "mocked" }),
        )
        .unwrap();
    assert_eq!(out["started"], false);
    assert_eq!(out["mocked"], json!(["A", "B", "C"]));
    assert_eq!(f.calls(), ["alias B mocks/B", "alias C mocks/C"]);

    // Going back, each alias is taken off once even after a failure.
    f.fail("alias B -", refused());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "state": "live" }),
    )
    .unwrap_err();
    assert_eq!(f.calls(), ["alias A -", "alias B -"]);
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "state": "live" }),
    )
    .unwrap();
    assert_eq!(f.calls(), ["alias B -", "alias C -"]);
}

/// Review 4427296687.
#[test]
fn a_refused_reload_is_a_failed_reload_and_the_next_reading_asks_again() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.fail("reload", refused());
    f.calls();
    f.call(
        "document_edit",
        json!({ "doc": "Shell.qml", "generation": 1, "text": "import QtQuick\n\nItem {}\n" }),
    )
    .unwrap();
    assert_eq!(f.calls(), ["reload"]);

    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["started"], false, "still open");
    f.call("preview_screenshot", json!({})).unwrap();
    assert_eq!(f.calls(), ["reload", "screenshot 800"]);
    let kinds = event_kinds(&f.events());
    assert!(kinds.contains(&"reload_failed".to_owned()), "{kinds:?}");
    assert!(!kinds.contains(&"ended".to_owned()), "{kinds:?}");
}

#[test]
fn a_reload_that_finds_the_preview_ended_ends_it() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.fail("reload", ended());
    f.call(
        "document_edit",
        json!({ "doc": "Shell.qml", "generation": 1, "text": "import QtQuick\n\nItem {}\n" }),
    )
    .unwrap();
    assert!(event_kinds(&f.events()).contains(&"ended".to_owned()));
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["started"], true);
}

#[test]
fn logs_that_cannot_be_read_end_the_preview_only_when_it_is_over() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.fail("logs", refused());
    let out = f.call("preview_events", json!({})).unwrap();
    assert_eq!(out["open"], true);
    assert!(event_kinds(out["events"].as_array().unwrap()).contains(&"reload_failed".to_owned()));
    f.calls();
    // The one reading after it asks for the reload it may have missed.
    f.call("preview_screenshot", json!({})).unwrap();
    assert_eq!(f.calls(), ["reload", "screenshot 800"]);

    f.fail("logs", ended());
    let out = f.call("preview_events", json!({})).unwrap();
    assert_eq!(out["open"], false);
    assert!(event_kinds(out["events"].as_array().unwrap()).contains(&"ended".to_owned()));
}

#[test]
fn a_preview_opens_only_on_a_document() {
    let f = Fixture::new();
    let e = f
        .call("preview_open", json!({ "doc": "mocks/App/Media/qmldir" }))
        .unwrap_err();
    assert_eq!(e["code"], "path_denied");
    assert_eq!(*f.opened.lock().unwrap(), 0);
}

/// Review 4429187157: a size past 1..=8192 on a side never reaches the host,
/// neither to start a preview nor to resize the open one.
#[test]
fn a_size_out_of_bounds_is_refused_before_start_or_resize() {
    let f = Fixture::new();
    let open = |width: u32, height: u32| {
        f.call(
            "preview_open",
            json!({ "doc": "Shell.qml", "size": { "width": width, "height": height } }),
        )
    };
    for (width, height) in [(0, 600), (800, 0), (8193, 600), (800, 100_000)] {
        assert_eq!(
            open(width, height).unwrap_err()["code"],
            "invalid_argument",
            "{width}x{height}"
        );
    }
    assert!(f.calls().is_empty());
    assert_eq!(*f.opened.lock().unwrap(), 0);

    assert_eq!(open(1, 1).unwrap()["started"], true);
    f.calls();
    open(8193, 600).unwrap_err();
    assert!(f.calls().is_empty());
    open(8192, 8192).unwrap();
    assert_eq!(f.calls(), ["resize 8192x8192"]);
}

fn declared(file: &str, line: u32, column: u32) -> Option<Declared> {
    Some(Declared {
        file: file.to_owned(),
        line,
        column,
    })
}

#[test]
fn a_point_selects_the_object_declared_where_the_preview_says() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    // `    Text { text: "x" }` is line 6; its type name starts at column 5.
    f.seen.lock().unwrap().declared = declared("Shell.qml", 6, 5);
    let out = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 12.5, "y": 40 } }),
        )
        .unwrap();
    assert_eq!(f.calls(), ["pick 12.5,40"]);
    assert_eq!(out["selection"][0]["path"], "root/1");
    assert_eq!(out["selection"][0]["type"], "Text");
    // The enclosing `Column` starts on line 4, not where the Text does.
    f.seen.lock().unwrap().declared = declared("Shell.qml", 4, 1);
    let out = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap();
    assert_eq!(out["selection"][0]["path"], "root");
}

#[test]
fn a_point_with_nothing_there_clears_the_selection() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.call(
        "node_select",
        json!({ "doc": "Shell.qml", "nodes": [{ "path": "root/1" }] }),
    )
    .unwrap();
    let out = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 5000, "y": 5000 } }),
        )
        .unwrap();
    assert_eq!(out["selection"], json!([]));
}

#[test]
fn a_point_in_a_file_of_a_block_selects_in_that_file() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.seen.lock().unwrap().declared = declared("App/Media/Player.qml", 3, 1);
    let out = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap();
    assert_eq!(out["selection"][0]["doc"], "App/Media/Player.qml");
    assert_eq!(out["selection"][0]["type"], "Item");
    f.seen.lock().unwrap().declared = declared("/elsewhere/Thing.qml", 1, 1);
    let refused = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap_err();
    assert_eq!(refused["details"]["kind"], "not_a_document");
}

#[test]
fn a_point_needs_a_preview_that_shows_the_text() {
    let f = Fixture::new();
    let refused = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap_err();
    assert_eq!(refused["details"]["kind"], "no_preview");
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let refused = f
        .call(
            "node_select",
            json!({ "doc": "Other.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap_err();
    assert_eq!(refused["details"]["kind"], "no_preview");
    f.seen.lock().unwrap().fail_next = true;
    f.call(
        "binding_set",
        json!({ "doc": "Shell.qml", "generation": 1, "node": { "path": "root/1" }, "name": "text", "value": "\"y\"" }),
    )
    .unwrap();
    f.seen.lock().unwrap().declared = declared("Shell.qml", 6, 5);
    let refused = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap_err();
    assert_eq!(refused["details"]["kind"], "preview_behind");
}
