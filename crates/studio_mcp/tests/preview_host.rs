//! What the preview tools ask of their host, with a host that records the
//! asking: the scenarios (`features/preview`) show the same through a real
//! `ljos`, and these pin the calls in between without one.

use std::sync::{Arc, Mutex};

use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::{Map, Value, json};
use studio_core::preview::{
    Click, Declared, Extent, Input, LogLine, Picture, PreviewError, PreviewHost, PreviewSpec,
    Reload, Rest, RestWatch, Size,
};
use studio_mcp::{Studio, StudioServer};

#[derive(Default)]
struct Seen {
    calls: Vec<String>,
    /// When each reload arrived, as the inspector would count it.
    reloads: Vec<std::time::Instant>,
    generation: u64,
    fail_next: bool,
    /// Calls that answer with an error, by how the call's note starts; each
    /// answers once.
    errors: Vec<(String, PreviewError)>,
    /// What `pick` answers.
    declared: Option<Declared>,
    /// The calls a second the host says it answers; 0 leaves Ljós's default.
    rate: u32,
    /// How far down the page reaches, as `extent` answers; none refuses.
    extent: Option<f64>,
    /// A full-width item this tall at the top of the page: it reaches its
    /// bottom while the window is shorter or taller, and once the window is
    /// exactly as tall the host leaves it out, as it does whatever is the
    /// window's size, so only `extent` is measured.
    tall: Option<f64>,
    /// The window's height, as the host was last opened or resized to.
    window: u32,
    /// What happens while a scroll or a swipe settles, outside Studio's lock.
    during_settle: Option<Box<dyn FnOnce() + Send>>,
    /// What another program does while a picture is taken.
    during_shot: Option<Box<dyn FnOnce() + Send>>,
    /// How many times `extent` was asked: the `n`th answers to the call
    /// `extent n`, so a test can fail one of a fit's measures.
    extents: usize,
    /// On the `n`th measure the `tall` item folds to this height first, as a
    /// page changes while it runs.
    folds: Option<(usize, f64)>,
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
    fn calls_per_second(&self) -> u32 {
        match self.0.lock().unwrap().rate {
            0 => studio_core::preview::DEFAULT_CALLS_PER_SECOND,
            rate => rate,
        }
    }

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
        self.0.lock().unwrap().window = spec.size.height;
        Ok(())
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        self.note("reload".to_owned())?;
        let mut seen = self.0.lock().unwrap();
        seen.reloads.push(std::time::Instant::now());
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

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        self.note(format!("resize {}x{}", size.width, size.height))?;
        self.0.lock().unwrap().window = size.height;
        Ok(())
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        self.note(format!("screenshot {max_side}"))?;
        let during = self.0.lock().unwrap().during_shot.take();
        if let Some(during) = during {
            during();
        }
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
                compiler: false,
            }]
        } else {
            Vec::new()
        };
        Ok((lines, 1))
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        self.answer("alive")
    }

    fn extent(&mut self) -> Result<Extent, PreviewError> {
        let n = {
            let mut seen = self.0.lock().unwrap();
            seen.extents += 1;
            if let Some((at, tall)) = seen.folds
                && at == seen.extents
            {
                seen.tall = Some(tall);
            }
            seen.extents
        };
        self.answer(&format!("extent {n}"))?;
        let seen = self.0.lock().unwrap();
        let reach = seen.extent.ok_or(PreviewError::Refused {
            code: "unsupported".to_owned(),
            message: "no extent set".to_owned(),
        })?;
        Ok(match seen.tall {
            Some(tall) if f64::from(seen.window) == tall => Extent {
                reach,
                window_sized: vec!["tall".to_owned()],
            },
            Some(tall) => Extent {
                reach: reach.max(tall),
                window_sized: Vec::new(),
            },
            None => Extent {
                reach,
                window_sized: Vec::new(),
            },
        })
    }

    fn click(&mut self, at: &Click) -> Result<(), PreviewError> {
        self.note(match at {
            Click::Id(id) => format!("click {id}"),
            Click::Point { x, y } => format!("click at {x},{y}"),
        })?;
        Ok(())
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        self.note(format!("pick {x},{y}"))?;
        Ok(self.0.lock().unwrap().declared.clone())
    }

    fn input(&mut self, input: &Input) -> Result<(), PreviewError> {
        self.note(match input {
            Input::Scroll { dy, .. } => format!("scroll {dy}"),
            Input::Swipe { dy, .. } => format!("swipe {dy}"),
            Input::Press(keys) => format!("press {keys}"),
            Input::Type(text) => format!("type {text}"),
        })
    }

    fn rest_watch(&self) -> Option<Box<dyn RestWatch>> {
        Some(Box::new(Settling(self.0.clone())))
    }

    fn close(&mut self) {
        let _ = self.note("close".to_owned());
    }
}

/// A page that comes to rest at once, after whatever `during_settle` does.
struct Settling(Arc<Mutex<Seen>>);

impl RestWatch for Settling {
    fn wait(&self, _: Option<u64>, _: u64) -> Result<Rest, PreviewError> {
        Err(PreviewError::NotOpen)
    }

    fn parked(
        &self,
        _: u64,
        _: u64,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Rest, PreviewError>> + Send>> {
        Box::pin(async { Err(PreviewError::NotOpen) })
    }

    fn settle(&self, _: u64) -> Result<(), PreviewError> {
        let during = self.0.lock().unwrap().during_settle.take();
        if let Some(during) = during {
            during();
        }
        Ok(())
    }
}

const SHELL: &str = "import QtQuick\nimport App.Media\n\nColumn { id: shell\n    Player { id: player1 }\n    Text { text: \"x\" }\n}\n";

struct Fixture {
    _dir: tempfile::TempDir,
    server: StudioServer,
    seen: Arc<Mutex<Seen>>,
    opened: Arc<Mutex<usize>>,
    /// Hears whenever a window would be told that what it shows changed.
    told: std::sync::mpsc::Receiver<()>,
}

impl Fixture {
    fn new() -> Self {
        Self::with(|_| {})
    }

    /// `arrange` changes the project before Studio opens it: after, Studio
    /// hears of it as of any other program's change.
    fn with(arrange: impl FnOnce(&std::path::Path)) -> Self {
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
        arrange(dir.path());
        let seen = Arc::new(Mutex::new(Seen::default()));
        let opened = Arc::new(Mutex::new(0));
        let mut studio = Studio::open(dir.path()).unwrap().with_previews(Arc::new({
            let seen = seen.clone();
            let opened = opened.clone();
            move |_| {
                *opened.lock().unwrap() += 1;
                Box::new(Recorder(seen.clone()))
            }
        }));
        let (tell, told) = std::sync::mpsc::channel();
        studio.watch(Arc::new(move || {
            let _ = tell.send(());
        }));
        Fixture {
            _dir: dir,
            server: StudioServer::new(studio),
            seen,
            opened,
            told,
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
fn a_window_hears_after_a_renamed_object_is_hidden_under_its_new_id() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.call(
        "node_arrange",
        json!({ "action": "hide", "doc": "Shell.qml", "nodes": [{ "id": "shell" }] }),
    )
    .unwrap();
    let seen = f.seen.clone();
    f.server.studio().lock().unwrap().watch(Arc::new(move || {
        seen.lock().unwrap().calls.push("told".to_owned());
    }));
    f.calls();
    f.call(
        "node_arrange",
        json!({ "action": "rename", "doc": "Shell.qml", "nodes": [{ "id": "shell" }], "name": "heading", "generation": 1 }),
    )
    .unwrap();
    let calls = f.calls();
    let wrote = calls
        .iter()
        .position(|c| c == "write heading.visible=false")
        .unwrap_or_else(|| panic!("never written: {calls:?}"));
    assert!(
        calls[wrote..].iter().any(|c| c == "told"),
        "nobody told after the write: {calls:?}"
    );
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

/// Opened again with nothing changed, a preview fitted to its content keeps
/// its height: it is not laid out at its screen's height and back up.
#[test]
fn reopening_a_preview_fitted_to_its_content_resizes_nothing() {
    let f = Fixture::new();
    f.seen.lock().unwrap().extent = Some(2000.0);
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "fit": "content" }),
        )
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 2000 }));
    f.calls();
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 2000 }));
    assert_eq!(f.calls(), Vec::<String>::new());
}

/// One whose page got shorter is laid out at its screen's height again and
/// measured there, so it gets shorter too.
#[test]
fn reopening_a_preview_fitted_to_a_page_that_got_shorter_makes_it_shorter() {
    let f = Fixture::new();
    f.seen.lock().unwrap().extent = Some(2000.0);
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap();
    f.calls();
    f.seen.lock().unwrap().extent = Some(1000.0);
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 1000 }));
    assert_eq!(f.calls(), ["resize 1280x800", "resize 1280x1000"]);
}

/// A page as tall as the preview was grown to, because of one item the
/// host now leaves out (it is the window's size), measures shorter than the
/// preview while nothing changed: opened again, it is not laid out at its
/// screen's height and back up.
#[test]
fn reopening_a_preview_grown_to_an_item_as_big_as_its_window_resizes_nothing() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "fit": "content" }),
        )
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 2000 }));
    f.calls();
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 2000 }));
    assert_eq!(f.calls(), Vec::<String>::new());
}

/// When that item gets shorter, the page measures differently, so the
/// preview is laid out from its screen again and gets shorter with it.
#[test]
fn reopening_a_preview_grown_to_an_item_that_got_shorter_makes_it_shorter() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap();
    f.calls();
    f.seen.lock().unwrap().tall = Some(1700.0);
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 1700 }));
    assert_eq!(f.calls(), ["resize 1280x800", "resize 1280x1700"]);
}

/// Folded to within the rest of the page, that item reaches no further
/// than the rest, so the page measures what it did; but the item is not
/// the window's size any more, so the page has changed and the preview is
/// laid out from its screen and fitted to the rest.
#[test]
fn reopening_a_preview_grown_to_an_item_folded_within_the_rest_fits_the_rest() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap();
    f.calls();
    f.seen.lock().unwrap().tall = Some(1000.0);
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 1500 }));
    assert_eq!(f.calls(), ["resize 1280x800", "resize 1280x1500"]);
}

/// A reopen compares what the page measures with what it measured when it
/// was last grown, and keeps the measure it compared: an item that folds
/// after that measure is seen by the next reopen, which lays the page out
/// again, and is not taken as what the unchanged page measures (review
/// 6027993410).
#[test]
fn an_item_folded_after_a_reopen_measured_the_page_is_seen_by_the_next_reopen() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap();
    f.calls();
    // Just after the reopen's first measure.
    let after = f.seen.lock().unwrap().extents + 2;
    f.seen.lock().unwrap().folds = Some((after, 1000.0));
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 1500 }));
    assert_eq!(f.calls(), ["resize 1280x800", "resize 1280x1500"]);
}

/// A measure refused partway through a reopen, after the page measured what
/// it did, leaves what it measured when it was last grown: the next reopen
/// of the unchanged page resizes nothing (review 6027419412).
#[test]
fn a_measure_refused_on_a_reopen_keeps_what_the_unchanged_page_measured() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap();
    f.calls();
    // The reopen measures the page to see whether it changed, and grows from
    // that measure: the one measure it makes is refused.
    let first = f.seen.lock().unwrap().extents + 1;
    f.fail(&format!("extent {first}"), refused());
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap_err();
    assert_eq!(
        f.seen.lock().unwrap().extents,
        first,
        "not the measure meant"
    );
    assert_eq!(f.calls(), Vec::<String>::new());
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 1280, "height": 2000 }));
    assert_eq!(f.calls(), Vec::<String>::new());
}

/// A host that cannot say how far its page reaches refuses a fit to
/// content, whether the preview starts with it or changes to it. The fit it
/// refused is not kept: the next open that names no fit is fitted to its
/// screen, not refused for the fit asked before.
#[test]
fn a_refused_fit_to_content_leaves_the_preview_fitted_to_its_screen() {
    let f = Fixture::new();
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap_err();
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "target": "phone" }),
        )
        .unwrap();
    assert_eq!(out["started"], false);
    assert_eq!(out["fit"], "screen");
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap_err();
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "target": "desktop" }),
        )
        .unwrap();
    assert_eq!(out["fit"], "screen");
    assert_eq!(out["size"], json!({ "width": 1280, "height": 800 }));
}

/// The clicks a window counts to know the preview's picture changed without
/// a reload.
fn clicks(f: &Fixture) -> u64 {
    let studio = f.server.studio();
    let mut studio = studio.lock().unwrap();
    studio.view("Shell.qml").preview.unwrap().clicks
}

/// A fit to content the host refuses after it grew the preview leaves it
/// fitted to its screen again, and so at its screen's size: not as tall as
/// the page it grew to while its fit says screen (review 7079316842).
#[test]
fn a_fit_refused_partway_puts_the_preview_back_to_its_screen() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    // Grown to the page's first measure, the fit's second is refused.
    let second = f.seen.lock().unwrap().extents + 2;
    f.fail(&format!("extent {second}"), refused());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap_err();
    assert_eq!(f.calls(), ["resize 1280x2000", "resize 1280x800"]);
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["fit"], "screen");
    assert_eq!(out["size"], json!({ "width": 1280, "height": 800 }));
    assert_eq!(f.calls(), Vec::<String>::new());
}

/// Put back to its screen, a preview the host would not resize again stays
/// as tall as it grew: a window is told its picture changed, as for a click,
/// and the next open resizes it (review 7079316842).
#[test]
fn a_fit_refused_partway_tells_a_window_of_the_size_it_left() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    let before = clicks(&f);
    let second = f.seen.lock().unwrap().extents + 2;
    f.fail(&format!("extent {second}"), refused());
    f.fail("resize 1280x800", refused());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap_err();
    assert_eq!(f.calls(), ["resize 1280x2000", "resize 1280x800"]);
    assert_eq!(clicks(&f), before + 1, "no window was told of the resize");
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["fit"], "screen");
    assert_eq!(out["size"], json!({ "width": 1280, "height": 800 }));
    assert_eq!(f.calls(), ["resize 1280x800"]);
}

/// A preview started fitted to content whose fit the host refuses after it
/// grew is kept, fitted to its screen as documented, and at its screen's
/// size: the next open that names no fit reuses it as it is.
#[test]
fn a_new_preview_whose_fit_is_refused_partway_is_left_at_its_screen() {
    let f = Fixture::new();
    {
        let mut seen = f.seen.lock().unwrap();
        seen.tall = Some(2000.0);
        seen.extent = Some(1500.0);
    }
    f.fail("extent 2", refused());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap_err();
    let resizes: Vec<String> = f
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("resize"))
        .collect();
    assert_eq!(resizes, ["resize 1280x2000", "resize 1280x800"]);
    let out = f
        .call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    assert_eq!(out["started"], false);
    assert_eq!(out["fit"], "screen");
    assert_eq!(out["size"], json!({ "width": 1280, "height": 800 }));
    assert_eq!(f.calls(), Vec::<String>::new());
}

/// A resize the host refused leaves the preview at the size it has, and
/// the screen it was asked for is not taken as reached: asked again, it is
/// resized, not answered with a size the host never had. Fitted to its
/// content, so the second ask is not laid out from the screen anyway.
#[test]
fn a_refused_resize_is_asked_again_and_not_answered_as_done() {
    let f = Fixture::new();
    f.seen.lock().unwrap().extent = Some(2000.0);
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "fit": "content" }),
    )
    .unwrap();
    f.calls();
    f.fail("resize", refused());
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "target": "phone" }),
    )
    .unwrap_err();
    f.calls();
    let out = f
        .call(
            "preview_open",
            json!({ "doc": "Shell.qml", "target": "phone" }),
        )
        .unwrap();
    assert_eq!(out["size"], json!({ "width": 390, "height": 2000 }));
    assert_eq!(f.calls(), ["resize 390x844", "resize 390x2000"]);
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
    // What the preview logged as it opened is read then, before any edit.
    assert_eq!(kinds, ["log", "reloaded", "reload_failed"]);
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

/// No second holds more reloads than the budget allows: 4 (preview.rs).
fn assert_within_the_budget(f: &Fixture) {
    let seen = f.seen.lock().unwrap();
    for (first, last) in seen.reloads.iter().zip(seen.reloads.iter().skip(4)) {
        assert!(
            last.duration_since(*first) >= std::time::Duration::from_secs(1),
            "five reloads in {:?}",
            last.duration_since(*first)
        );
    }
}

/// The `i`th write after the preview opened, so of generation `i + 1`.
fn set_text(f: &Fixture, i: usize) {
    f.call(
        "binding_set",
        json!({ "doc": "Shell.qml", "generation": i + 1, "node": { "path": "root/1" }, "name": "text", "value": format!("\"{i}\"") }),
    )
    .unwrap();
}

#[test]
fn a_burst_with_a_picture_after_every_write_stays_within_the_budget() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    for i in 0..20 {
        set_text(&f, i);
        f.call("preview_screenshot", json!({})).unwrap();
    }
    assert_within_the_budget(&f);
    // Each look saw the write before it, budget or no budget.
    assert_eq!(f.seen.lock().unwrap().reloads.len(), 20);
}

/// The picture is labelled with the text it was taken of: an edit that lands
/// while it is taken is newer than its pixels, so a window keeping the
/// picture still sees it is behind.
#[test]
fn a_picture_is_labelled_with_the_text_before_an_edit_that_lands_while_it_is_taken() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let before = f
        .server
        .studio()
        .lock()
        .unwrap()
        .session
        .generation("Shell.qml")
        .unwrap();
    let shell = f._dir.path().join("Shell.qml");
    f.seen.lock().unwrap().during_shot = Some(Box::new(move || {
        std::fs::write(shell, SHELL.replace("\"x\"", "\"y\"")).unwrap();
    }));
    let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run]));
    let (_, shown) = f.server.shown_picture(&ctx, json!({})).unwrap();
    assert_eq!(shown.unwrap().text, before);
    // The edit is the session's once it next reads the file.
    let after = f
        .server
        .studio()
        .lock()
        .unwrap()
        .session
        .generation("Shell.qml")
        .unwrap();
    assert_eq!(after, before + 1);
}

#[test]
fn a_window_fetching_pictures_in_a_burst_stays_within_the_budget_and_ends_up_shown() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run]));
    let mut last_write = std::time::Instant::now();
    for i in 0..20 {
        last_write = std::time::Instant::now();
        set_text(&f, i);
        f.server.shown_picture(&ctx, json!({})).unwrap();
    }
    assert_within_the_budget(&f);
    // Whatever the timer has done by now, a reading shows the last write.
    f.call("preview_events", json!({})).unwrap();
    assert_within_the_budget(&f);
    assert!(*f.seen.lock().unwrap().reloads.last().unwrap() >= last_write);
}

/// The budget is the host's: a preview that answers more calls a second
/// reloads more of a burst at once.
#[test]
fn a_host_that_answers_more_calls_reloads_more_of_a_burst() {
    let counted = |rate| {
        let f = Fixture::new();
        f.seen.lock().unwrap().rate = rate;
        f.call("preview_open", json!({ "doc": "Shell.qml" }))
            .unwrap();
        f.calls();
        for i in 0..40 {
            set_text(&f, i);
        }
        f.calls().len()
    };
    let (default, granted) = (counted(0), counted(50));
    assert!(
        default < granted,
        "{default} reloads at the default rate, {granted} at 50 a second"
    );
    assert!(
        granted < 40,
        "{granted} reloads for 40 writes at 50 a second"
    );
}

#[test]
fn a_preview_that_does_not_open_takes_the_last_ones_problems_with_it() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let before = f.call("preview_events", json!({})).unwrap();
    assert_eq!(before["problems"][0]["message"], "a warning", "{before}");
    while f.told.try_recv().is_ok() {}
    f.fail("open Other.qml", PreviewError::Start("no".to_owned()));
    f.call("preview_open", json!({ "doc": "Other.qml" }))
        .unwrap_err();
    f.told
        .try_recv()
        .expect("the window was told the last preview ended");
    let after = f.call("preview_events", json!({})).unwrap();
    assert_eq!(after["open"], false);
    assert!(after.get("problems").is_none(), "{after}");
}

#[test]
fn no_problems_are_held_once_no_preview_is_open_however_it_ended() {
    for how in ["a reload", "a mock switch", "closed"] {
        let f = Fixture::new();
        f.call("preview_open", json!({ "doc": "Shell.qml" }))
            .unwrap();
        if how == "closed" {
            let close = f.server.studio().lock().unwrap().preview_closer();
            close();
        } else {
            f.seen.lock().unwrap().fail_next = true;
            f.fail("logs", ended());
            if how == "a reload" {
                f.call(
                    "binding_set",
                    json!({ "doc": "Shell.qml", "generation": 1, "node": { "path": "root/1" }, "name": "text", "value": "\"y\"" }),
                )
                .unwrap();
            } else {
                // What this answers is the next test's.
                let _ = f.call(
                    "preview_open",
                    json!({ "doc": "Shell.qml", "state": "mocked" }),
                );
            }
        }
        f.fail("open Other.qml", PreviewError::Start("no".to_owned()));
        f.call("preview_open", json!({ "doc": "Other.qml" }))
            .unwrap_err();
        let after = f.call("preview_events", json!({})).unwrap();
        assert_eq!(after["open"], false, "{how}");
        assert!(after.get("problems").is_none(), "{how}: {after}");
    }
}

#[test]
fn a_preview_that_ends_as_it_opens_answers_ended() {
    for (reused, appearance) in [(false, false), (false, true), (true, false), (true, true)] {
        let case = format!("reused {reused}, appearance {appearance}");
        let f = Fixture::new();
        let mut args = json!({ "doc": "Shell.qml" });
        if reused {
            f.call("preview_open", args.clone()).unwrap();
            // A change in place, whose logs are read.
            args["state"] = json!("mocked");
        }
        if appearance {
            args["appearance"] = json!("dark");
        }
        f.fail("logs", ended());
        let refused = f.call("preview_open", args).expect_err(&case);
        assert_eq!(refused["details"]["kind"], "ended", "{case}");
        assert_eq!(
            refused["message"], "the preview has ended: the process exited",
            "{case}"
        );
        let after = f.call("preview_events", json!({})).unwrap();
        assert_eq!(after["open"], false, "{case}");
        let kinds = event_kinds(after["events"].as_array().unwrap());
        assert_eq!(
            kinds.iter().filter(|k| *k == "ended").count(),
            1,
            "{case}: {kinds:?}"
        );
    }
}

#[test]
fn a_burst_shows_once_the_budget_allows_with_nobody_looking() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    for i in 0..20 {
        f.call(
            "binding_set",
            json!({ "doc": "Shell.qml", "generation": i + 1, "node": { "path": "root/1" }, "name": "text", "value": format!("\"{i}\"") }),
        )
        .unwrap();
    }
    f.calls();
    while f.told.try_recv().is_ok() {}
    let began = std::time::Instant::now();
    // A window hears of the catch-up as of any change, and the preview was
    // asked for the burst's last state though nothing read it.
    f.told
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the window was told of the catch-up");
    assert_eq!(f.calls(), ["reload"]);
    assert!(
        began.elapsed() <= std::time::Duration::from_millis(1100),
        "caught up after {:?}",
        began.elapsed()
    );
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
    assert_eq!(f.calls(), ["read player1 playing,state"]);
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
        ["write player1.playing=false", "read player1 playing,state"]
    );
}

#[test]
fn an_object_that_is_not_a_block_has_only_its_variant() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    let out = f
        .call("preview_state", json!({ "node": { "path": "root" } }))
        .unwrap();
    assert_eq!(out.get("block"), None);
    assert_eq!(out["state"], json!({}));
    assert_eq!(out["variant"], "");
    assert_eq!(f.calls(), ["read shell state"]);
    let refused = f
        .call(
            "preview_state",
            json!({ "node": { "path": "root" }, "set": { "state": "open" } }),
        )
        .unwrap_err();
    assert_eq!(refused["details"]["kind"], "not_a_block");
}

#[test]
fn a_click_comes_after_set_and_before_the_read() {
    let f = Fixture::new();
    f.call(
        "preview_open",
        json!({ "doc": "Shell.qml", "state": "mocked" }),
    )
    .unwrap();
    f.calls();
    f.call(
        "preview_state",
        json!({ "node": { "id": "player1" }, "set": { "playing": true }, "click": { "id": "player1" } }),
    )
    .unwrap();
    f.call(
        "preview_state",
        json!({ "node": { "path": "root" }, "click_at": { "x": 4, "y": 8.5 } }),
    )
    .unwrap();
    assert_eq!(
        f.calls(),
        [
            "write player1.playing=true",
            "click player1",
            "read player1 playing,state",
            "click at 4,8.5",
            "read shell state",
        ]
    );
}

#[test]
fn a_click_needs_an_object_with_an_id_and_one_place() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    f.calls();
    let no_id = f
        .call(
            "preview_state",
            json!({ "node": { "path": "root" }, "click": { "path": "root/1" } }),
        )
        .unwrap_err();
    assert_eq!(no_id["code"], "invalid_argument");
    let both = f
        .call(
            "preview_state",
            json!({ "node": { "path": "root" }, "click": { "id": "player1" }, "click_at": { "x": 1, "y": 1 } }),
        )
        .unwrap_err();
    assert_eq!(both["code"], "invalid_argument");
    assert!(f.calls().is_empty());
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
fn three_mocks(dir: &std::path::Path) {
    for module in ["A", "B", "C"] {
        let path = dir.join("mocks").join(module);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("qmldir"), format!("module {module}\n")).unwrap();
    }
    std::fs::remove_dir_all(dir.join("mocks/App")).unwrap();
}

/// Review 4427296683.
#[test]
fn a_failure_on_the_second_of_three_aliases_keeps_the_record_true() {
    let f = Fixture::with(three_mocks);
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
fn a_point_after_a_burst_is_read_from_the_last_edit() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let mut last_write = std::time::Instant::now();
    for i in 0..20 {
        last_write = std::time::Instant::now();
        set_text(&f, i);
    }
    f.seen.lock().unwrap().declared = declared("Shell.qml", 6, 5);
    let out = f
        .call(
            "node_select",
            json!({ "doc": "Shell.qml", "at": { "x": 1, "y": 1 } }),
        )
        .unwrap();
    // The line the preview names is of the text the session holds: the
    // reload of the last write came before the question.
    let calls = f.calls();
    assert_eq!(calls.last().unwrap(), "pick 1,1");
    assert!(*f.seen.lock().unwrap().reloads.last().unwrap() >= last_write);
    assert_eq!(out["selection"][0]["path"], "root/1");
    assert_within_the_budget(&f);
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

/// A read waits for a scroll to settle outside Studio's lock, so another call
/// may open another preview meanwhile. What it would read then is another
/// app's, so it reads nothing and says why.
#[test]
fn a_read_after_a_scroll_reads_nothing_from_a_preview_opened_while_it_settled() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let server = f.server.clone();
    f.seen.lock().unwrap().during_settle = Some(Box::new(move || {
        let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run]));
        server
            .call("preview_open", &ctx, json!({ "doc": "Other.qml" }))
            .unwrap();
    }));
    f.calls();
    let refused = f
        .call(
            "preview_state",
            json!({
                "doc": "Shell.qml",
                "node": { "id": "player1" },
                "scroll": { "x": 10, "y": 10, "dy": -3 },
                "read": ["playing"]
            }),
        )
        .unwrap_err();
    assert_eq!(refused["code"], "stale_ref", "{refused}");
    assert_eq!(refused["details"]["kind"], "preview_replaced", "{refused}");
    let calls = f.calls();
    assert!(
        !calls.iter().any(|c| c.starts_with("read ")),
        "read the other preview: {calls:?}"
    );
    // The preview opened meanwhile is not taken for one that failed: opening
    // it again finds it running.
    f.call("preview_open", json!({ "doc": "Other.qml" }))
        .unwrap();
    assert_eq!(*f.opened.lock().unwrap(), 2);
}

/// The same document switched to its mocks while a scroll settled, in place:
/// what is read is the mock's, so it says so (review 6027291981).
#[test]
fn a_read_after_a_scroll_says_mocked_when_mocks_came_in_while_it_settled() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let server = f.server.clone();
    f.seen.lock().unwrap().during_settle = Some(Box::new(move || {
        let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run]));
        let out = server
            .call(
                "preview_open",
                &ctx,
                json!({ "doc": "Shell.qml", "state": "mocked" }),
            )
            .unwrap();
        assert_eq!(out.structured["started"], false, "not switched in place");
    }));
    let out = f
        .call(
            "preview_state",
            json!({
                "node": { "id": "player1" },
                "scroll": { "x": 10, "y": 10, "dy": -3 }
            }),
        )
        .unwrap();
    assert_eq!(out["block"], "App.Media.Player");
    assert_eq!(out["mocked"], true, "{out}");
}

/// The object a read names removed while a scroll settled, and its `id`
/// given to another: the read is of another object, so it reads nothing and
/// says why (review 6027419412).
#[test]
fn a_read_after_a_scroll_reads_nothing_when_its_object_was_replaced_while_it_settled() {
    let f = Fixture::new();
    f.call("preview_open", json!({ "doc": "Shell.qml" }))
        .unwrap();
    let server = f.server.clone();
    f.seen.lock().unwrap().during_settle = Some(Box::new(move || {
        let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run]));
        let removed = server
            .call(
                "node_arrange",
                &ctx,
                json!({ "action": "remove", "doc": "Shell.qml", "nodes": [{ "id": "player1" }] }),
            )
            .unwrap_or_else(|e| panic!("the remove: {}", e.to_value()))
            .structured;
        // A path names an object in the generation it was read at: the
        // `Text` is first once the player is gone.
        let generation = removed["generation"]
            .as_u64()
            .unwrap_or_else(|| panic!("the remove's generation: {removed}"));
        server
            .call(
                "node_arrange",
                &ctx,
                json!({ "action": "rename", "doc": "Shell.qml", "generation": generation, "nodes": [{ "path": "root/0" }], "name": "player1" }),
            )
            .unwrap_or_else(|e| panic!("the rename: {}", e.to_value()));
    }));
    f.calls();
    let refused = f
        .call(
            "preview_state",
            json!({
                "node": { "id": "player1" },
                "scroll": { "x": 10, "y": 10, "dy": -3 },
                "read": ["text"]
            }),
        )
        .unwrap_err();
    assert_eq!(refused["code"], "stale_ref", "{refused}");
    assert_eq!(refused["details"]["kind"], "node_replaced", "{refused}");
    let calls = f.calls();
    assert!(
        !calls.iter().any(|c| c.starts_with("read ")),
        "read the other object: {calls:?}"
    );
}
