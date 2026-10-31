//! Studio's window budgets (DESIGN.md section 6), measured in wall time on
//! the shell Studio ships, in-process on a window without a platform:
//!
//! - cold start: a 200-file project opened, the shell made and its first
//!   frame shown with the documents listed;
//! - selection → inspector: a click on a layer until the inspector shows
//!   that object's bindings;
//! - inspector edit → picture: Enter in the inspector until the frame draws
//!   the preview's next generation, back to back and at a person's pace
//!   (needs `LJOS`, the `ljos` program the preview runs);
//! - `theme_set` → picture: Enter in the tokens panel until the frame draws
//!   the preview's next generation (needs `LJOS`);
//! - the self-reskin, on Studio's window watching a shell of the person's
//!   as `studio --shell` runs it: the window opened on a shell that
//!   throws until the built-in one is shown; a save of Studio's own theme
//!   file until the window is drawn in it; and what the watched window
//!   costs while nobody touches it, in CPU time;
//! - a drag across the canvas's frame, each pointer move timed until the
//!   window is at rest (its frame's work, before the GPU draws it). The
//!   canvas has no gesture that drags a node, so this is the frame cost of
//!   what a drag over the frame does today: it pans the canvas.
//!
//! `budgets sample DIR DOC LAYER FIELD [ROUNDS]` measures a project of
//! your own instead of the made-up one: cold start on DIR (which has a
//! `Main.qml`), DOC opened, the layer labelled LAYER selected, ROUNDS
//! inspector edits of its numeric FIELD to a new picture, one per 250 ms,
//! and the drag.
//!
//! The window's clock is a test clock, so a frame costs only its work; the
//! tool calls, the preview's reload and the picture's fetch and decode run
//! for real. Each edit's time is split where it went (the write, the
//! preview's reload and its answer, the picture's fetch, the frame), and
//! the slowest edits of each round are printed with that breakdown, so a
//! slow sample says why it was slow. Run it in a release build:
//!
//! ```text
//! LJOS=…/ljos cargo run --release -p studio --example budgets [-- ROUNDS]
//! ```

use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow, bail};
use ljos::item::MouseButtons;
use ljos::prelude::*;
use ljos::testing::AccessibleNode;
use ljos_dev::{DevSession, Host, Report};
use ljos_drive::{Chord, Stage};
use ljos_runtime::SystemClock;
use studio_core::preview::{
    Declared, LogLine, Picture, PreviewError, PreviewHost, PreviewSpec, Reload, Size,
};
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

const WIDTH: f64 = 1440.0;
const HEIGHT: f64 = 900.0;
const FILES: usize = 200;
/// A brisk person's pace through the inspector; the back-to-back round
/// shows what the preview's call rate leaves of a faster one.
const PACE: Duration = Duration::from_millis(250);
/// Longer than the preview's reload budget takes to allow another reload.
const CATCH_UP: Duration = Duration::from_millis(1500);
/// The slowest edits of a round whose breakdown is printed.
const SLOWEST: usize = 3;
/// Two canvases the theme edits switch between, each passing the
/// contrast Ljós asks of the roles drawn on it.
const CANVASES: [&str; 2] = ["#ffffff", "#fcfcfc"];
/// How long the idle window is left alone.
const IDLE: Duration = Duration::from_secs(5);
/// The value the root's state changes, as the inspector names it: an object
/// the selection rounds leave alone, so a state that is always on does not
/// hide their edits of `width`.
const STATE_FIELD: &str = "card7.width";

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("sample") {
        let [_, dir, doc, layer, field, rest @ ..] = args.as_slice() else {
            bail!("usage: budgets sample DIR DOC LAYER FIELD [ROUNDS]");
        };
        let rounds = match rest.first() {
            Some(n) => n.parse().context("ROUNDS is a number")?,
            None => 20,
        };
        return sample(Path::new(dir), doc, layer, field, rounds);
    }
    let rounds: usize = match std::env::args().nth(1) {
        Some(n) => n.parse().context("ROUNDS is a number")?,
        None => 20,
    };
    let work = tempfile::tempdir()?;
    let project = work.path().join("project");
    let state = work.path().join("state");
    write_project(&project)?;

    let mut starts = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        let window = Window::open(&project, &state)?;
        starts.push(started.elapsed());
        drop(window);
    }
    report("cold start, 200 files", &mut starts);

    reskin(work.path(), rounds)?;

    let mut window = Window::open(&project, &state)?;
    window.open_document("Main.qml")?;

    let mut selections = Vec::new();
    for round in 0..rounds {
        let label = if round % 2 == 0 {
            "Rectangle card0"
        } else {
            "Rectangle card1"
        };
        let row = window.layer(label)?;
        let started = Instant::now();
        window.click(&row)?;
        window.inspector_field("color")?;
        selections.push(started.elapsed());
    }
    report("selection -> inspector shown", &mut selections);

    if std::env::var_os("LJOS").is_none() {
        println!("inspector edit -> new picture: not measured, LJOS is not set");
        return Ok(());
    }
    window.wait_for_picture(1)?;
    let edits = window.edits("width", rounds, 100, None)?;
    report_edits("inspector edit -> new picture, back to back", edits);
    // The back-to-back calls leave the inspector's one-second window full;
    // the paced edits start once it has emptied.
    std::thread::sleep(Duration::from_secs(1));
    let edits = window.edits("width", rounds, 200, Some(PACE))?;
    report_edits(
        &format!(
            "inspector edit -> new picture, one per {} ms",
            PACE.as_millis()
        ),
        edits,
    );

    std::thread::sleep(Duration::from_secs(1));
    let row = window.layer("Rectangle root")?;
    window.click(&row)?;
    let state = window.node_with("Inspector", "State on", |n| n.role == "button")?;
    window.click(&state)?;
    let edits = window.edits(STATE_FIELD, rounds, 20, Some(PACE))?;
    report_edits(
        &format!(
            "state edit in the inspector -> new picture, one per {} ms",
            PACE.as_millis()
        ),
        edits,
    );

    std::thread::sleep(Duration::from_secs(1));
    let mut edits = window.token_edits(rounds, PACE)?;
    report(
        &format!(
            "theme_set in the tokens panel -> new picture, one per {} ms",
            PACE.as_millis()
        ),
        &mut edits,
    );

    std::thread::sleep(Duration::from_secs(1));
    let mut moves = window.drag_frames(DRAG_MOVES)?;
    report(
        "drag across the canvas's frame, per pointer move",
        &mut moves,
    );
    Ok(())
}

/// Pointer moves in one drag: four seconds of a hand at 60 Hz.
const DRAG_MOVES: usize = 240;

/// `budgets sample`: the numbers on a project of the person's own.
fn sample(dir: &Path, doc: &str, layer: &str, field: &str, rounds: usize) -> anyhow::Result<()> {
    if std::env::var_os("LJOS").is_none() {
        bail!("budgets sample needs LJOS, the `ljos` program the preview runs");
    }
    let work = tempfile::tempdir()?;
    let state = work.path().join("state");
    let mut starts = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        let window = Window::open(dir, &state)?;
        starts.push(started.elapsed());
        drop(window);
    }
    report(&format!("cold start, {}", dir.display()), &mut starts);

    let mut window = Window::open(dir, &state)?;
    window.document = dir.join(doc);
    window.open_document(doc)?;
    window.wait_for_picture(1)?;
    let row = window.layer(layer)?;
    window.click(&row)?;
    let first = window
        .inspector_value(field)?
        .parse::<f64>()
        .with_context(|| format!("{field} is not a number in the inspector"))?;
    // Back and forth around the value written, so the design stays as it was.
    let edits = window.edits_between(field, rounds, first, Some(PACE))?;
    report_edits(
        &format!(
            "{doc}: inspector edit of {field} -> new picture, one per {} ms",
            PACE.as_millis()
        ),
        edits,
    );
    std::thread::sleep(Duration::from_secs(1));
    let mut moves = window.drag_frames(DRAG_MOVES)?;
    report(
        &format!("{doc}: drag across the canvas's frame, per pointer move"),
        &mut moves,
    );
    Ok(())
}

/// The self-reskin's numbers, below `work`.
fn reskin(work: &Path, rounds: usize) -> anyhow::Result<()> {
    let builtin = work.join("builtin");
    studio_blocks::shell::write(&builtin)?;
    let theme = work.join("theme.json");

    let broken = work.join("broken");
    studio_blocks::shell::seed(&broken)?;
    let shell = broken.join("Shell.qml");
    let text = std::fs::read_to_string(&shell)?.replacen(
        "id: shell",
        "id: shell\n    Component.onCompleted: { throw new Error(\"no shell today\") }",
        1,
    );
    std::fs::write(&shell, text)?;
    let mut opens = Vec::new();
    for _ in 0..rounds {
        let started = Instant::now();
        let window = Watched::open(&broken, &builtin, &theme)?;
        if window.loader("status").as_number() != Some(2.0) {
            bail!(
                "the window does not show the built-in shell: {:?}",
                window.loader("error")
            );
        }
        opens.push(started.elapsed());
    }
    report(
        "window on a shell that throws -> built-in shown",
        &mut opens,
    );

    let own = work.join("own");
    studio_blocks::shell::seed(&own)?;
    let mut window = Watched::open(&own, &builtin, &theme)?;
    if window.loader("status").as_number() != Some(1.0) {
        bail!(
            "the window does not show the person's shell: {:?}",
            window.loader("error")
        );
    }
    let mut saves = Vec::new();
    for round in 0..rounds {
        std::thread::sleep(PACE);
        let canvas = CANVASES[round % 2];
        let shown = window.stage.state.shown;
        let started = Instant::now();
        // As `theme_set` saves it: written beside and renamed over.
        let tmp = work.join("theme.json.tmp");
        std::fs::write(
            &tmp,
            format!("{{ \"Colors\": {{ \"canvas\": {{ \"light\": \"{canvas}\" }} }} }}\n"),
        )?;
        std::fs::rename(&tmp, &theme)?;
        window.take_theme()?;
        let drawn = window
            .stage
            .engine
            .read_by_name(window.root(), "color")
            .to_js_string();
        if !drawn.eq_ignore_ascii_case(canvas) || window.stage.state.shown == shown {
            bail!("the window is drawn with {drawn}, not {canvas}");
        }
        saves.push(started.elapsed());
    }
    report(
        &format!(
            "Studio's theme file saved -> window repainted, one per {} ms",
            PACE.as_millis()
        ),
        &mut saves,
    );

    let frames = window.stage.state.shown;
    let before = cpu_time()?;
    std::thread::sleep(IDLE);
    let spent = cpu_time()? - before;
    let woken = window.woke.try_iter().count();
    window.rest()?;
    println!(
        "watched window left alone {} s: {:.0} ms CPU, {} wakes, {} frames",
        IDLE.as_secs(),
        ms(spent),
        woken,
        window.stage.state.shown - frames
    );
    Ok(())
}

/// The CPU time this process has spent, in user and kernel mode.
fn cpu_time() -> anyhow::Result<Duration> {
    let stat = std::fs::read_to_string("/proc/self/stat")?;
    // The fields after the command, which is in parentheses and may hold
    // spaces; utime and stime are the 14th and 15th of the whole line.
    let rest = stat
        .rsplit_once(')')
        .ok_or(anyhow!("no command in /proc/self/stat"))?
        .1;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let ticks: u64 = fields[11].parse::<u64>()? + fields[12].parse::<u64>()?;
    // Linux counts them in USER_HZ, 100 a second on every architecture it
    // reports to user space.
    Ok(Duration::from_millis(ticks * 10))
}

/// Studio's window as `studio --shell` runs it: the person's shell, Studio's
/// beneath it, watched with Studio's theme file, pumped when the watcher
/// wakes it.
struct Watched {
    /// Dropped first: the watcher's thread ends before the engine goes.
    session: DevSession,
    clock: SystemClock,
    woke: mpsc::Receiver<()>,
    stage: Stage,
}

impl Watched {
    fn open(shell: &Path, builtin: &Path, theme: &Path) -> anyhow::Result<Watched> {
        let engine = ljos::engine();
        studio_blocks::register(&engine);
        let mut stage = Stage::new(&engine, WIDTH, HEIGHT);
        let user_shell = studio_blocks::shell::url(&shell.join("Shell.qml"))?;
        let (tx, woke) = mpsc::channel();
        let session = DevSession::open_roots_themed(
            &engine,
            &[shell, builtin],
            &["Window"],
            vec![("userShell".to_owned(), user_shell.as_str().into())],
            ljos_dev::Aliases::default(),
            Some(theme),
            ljos::dev::EngineOracle::from_engine(&engine),
            Arc::new(move || {
                let _ = tx.send(());
            }),
        )?;
        let root = session
            .root()
            .ok_or(anyhow!("the watched shell made no window"))?;
        stage.scene.set_root(&engine, root)?;
        let mut window = Watched {
            session,
            clock: SystemClock::new(),
            woke,
            stage,
        };
        window.rest()?;
        Ok(window)
    }

    fn root(&self) -> ObjectId {
        self.stage.scene.root().expect("the window shows its root")
    }

    fn loader(&self, name: &str) -> Value {
        ljos_drive::find_below(&self.stage.engine, self.root(), "shellLoader")
            .map_or(Value::Null, |loader| {
                self.stage.engine.read_by_name(loader, name)
            })
    }

    /// Waits for the watcher to see the theme file saved, pumps the
    /// session until it has put the theme on, and draws the frame.
    fn take_theme(&mut self) -> anyhow::Result<()> {
        loop {
            self.woke
                .recv_timeout(Duration::from_secs(30))
                .map_err(|_| anyhow!("the window took no theme in 30 s"))?;
            let reports = self.session.pump(
                &self.stage.engine,
                &self.clock,
                &mut SceneHost(&mut self.stage.scene),
            );
            if let Some(failed) = reports.iter().find(|r| matches!(r, Report::Failed { .. })) {
                bail!("the theme did not apply: {failed:?}");
            }
            if reports.iter().any(|r| matches!(r, Report::Theme { .. })) {
                return self.rest();
            }
        }
    }

    fn rest(&mut self) -> anyhow::Result<()> {
        let mut driver = self.stage.drive();
        driver.settle()?;
        driver.finish_promised()?;
        Ok(())
    }
}

/// The scene a reload shows its new root in.
struct SceneHost<'a>(&'a mut ljos_quick::QuickScene);

impl Host for SceneHost<'_> {
    fn window(&self) -> Option<ObjectId> {
        self.0.root()
    }

    fn show(&mut self, engine: &Engine, _old: Option<ObjectId>, new: ObjectId) {
        self.0
            .set_root(engine, new)
            .expect("Studio's window is an Item");
    }
}

/// `Main.qml` with a few objects, and 199 components beside it in modules
/// of their own, as a project of that size is laid out. The modules sort
/// after `Main.qml`, so the documents list shows it without a scroll.
fn write_project(dir: &Path) -> anyhow::Result<()> {
    let mut main = String::from(
        "import QtQuick\n\nRectangle {\n    id: root\n    width: 400; height: 300\n    color: \"white\"\n    states: [\n        State {\n            name: \"on\"\n            when: true\n            PropertyChanges {\n                target: card7\n                width: 40\n            }\n        }\n    ]\n",
    );
    for i in 0..8 {
        main += &format!(
            "\n    Rectangle {{\n        id: card{i}\n        x: {}; y: 10; width: 40; height: 40\n        color: \"grey\"\n    }}\n",
            10 + i * 48
        );
    }
    main += "}\n";
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("Main.qml"), main)?;
    for i in 0..FILES - 1 {
        let module = dir.join(format!("parts/Part{}", i / 20));
        std::fs::create_dir_all(&module)?;
        std::fs::write(
            module.join(format!("Part{i}.qml")),
            format!(
                "import QtQuick\n\nItem {{\n    id: part\n    property string title: \"Part {i}\"\n    width: 100; height: 40\n\n    Text {{ text: part.title }}\n}}\n"
            ),
        )?;
    }
    Ok(())
}

fn report(what: &str, samples: &mut [Duration]) {
    samples.sort();
    let at = |p: f64| samples[((samples.len() - 1) as f64 * p).round() as usize];
    println!(
        "{what}: p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms over {}",
        ms(at(0.5)),
        ms(at(0.95)),
        ms(samples[samples.len() - 1]),
        samples.len()
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The percentiles of a round of edits, then where the time of its
/// slowest went.
fn report_edits(what: &str, edits: Vec<Edit>) {
    let mut samples: Vec<Duration> = edits.iter().map(|e| e.took).collect();
    report(what, &mut samples);
    let mut slowest: Vec<&Edit> = edits.iter().collect();
    slowest.sort_by_key(|e| std::cmp::Reverse(e.took));
    for edit in slowest.into_iter().take(SLOWEST) {
        println!("  {:7.1} ms  {}", ms(edit.took), edit.breakdown());
    }
}

/// A step of an edit's way to the screen, as the preview's host or the
/// session saw it, off the UI thread.
#[derive(Clone, Debug)]
enum Step {
    /// The document's new text was in its file beside it.
    Written,
    /// That file was renamed over the document.
    Renamed,
    /// The session told its watchers of a write, after the write's reload.
    Told,
    ReloadAsked,
    /// The reload's answer: `applied` (with the generation), `unchanged`
    /// or `failed`.
    ReloadAnswered(String),
    PictureAsked,
    /// The picture's PNG, in bytes.
    PictureAnswered(usize),
}

type Steps = Arc<Mutex<Vec<(Instant, Step)>>>;

/// One edit: Enter until the frame drew the reload, and what happened in
/// between.
struct Edit {
    took: Duration,
    steps: Vec<(Duration, Step)>,
    /// When the window was at rest after Enter: the gesture's call, the
    /// reload and the picture it promised are done.
    rested: Duration,
    /// How many times the harness looked again before the frame drew it.
    looks: usize,
    /// When the document's text was written (its mtime: Studio writes a
    /// file beside it and renames it over) and renamed into place (its
    /// inode's ctime), as the file system tells them, since Enter. Its
    /// clock is a coarse one, a few milliseconds behind: a step of the
    /// write is placed to within that, which is enough for a slow one.
    written: Option<(Duration, Duration)>,
    /// What the preview wrote to its log meanwhile: its own account of
    /// each reload (compile, apply, and when it was shown after the save).
    child: Vec<String>,
}

impl Edit {
    /// Each step with the time since the one before, then the frame.
    fn breakdown(&self) -> String {
        let mut out = Vec::new();
        let mut last = Duration::ZERO;
        let mut steps = self.steps.clone();
        if let Some((data, renamed)) = self.written {
            steps.push((data, Step::Written));
            steps.push((renamed, Step::Renamed));
            // Stable: a step at the same time as a write stays after it.
            steps.sort_by_key(|(at, _)| *at);
        }
        for (at, step) in &steps {
            let name = match step {
                Step::Written => "text written".to_owned(),
                Step::Renamed => "renamed into place".to_owned(),
                Step::Told => "told".to_owned(),
                Step::ReloadAsked => "reload asked".to_owned(),
                Step::ReloadAnswered(kind) => format!("reload {kind}"),
                Step::PictureAsked => "picture asked".to_owned(),
                Step::PictureAnswered(bytes) => format!("picture of {bytes} B"),
            };
            out.push(format!("+{:.1} {name}", ms(at.saturating_sub(last))));
            last = *at;
        }
        out.push(format!(
            "+{:.1} at rest",
            ms(self.rested.saturating_sub(last))
        ));
        out.push(format!(
            "+{:.1} frame drawn after {} more looks",
            ms(self.took.saturating_sub(self.rested)),
            self.looks
        ));
        let mut out = out.join(", ");
        for line in &self.child {
            out.push_str("\n             preview: ");
            out.push_str(line);
        }
        out
    }
}

/// A preview host that notes when the session asked it to reload and for
/// a picture, and how each answered.
struct Timed {
    host: NativeHost,
    steps: Steps,
}

impl Timed {
    fn note(&self, step: Step) {
        self.steps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((Instant::now(), step));
    }
}

impl PreviewHost for Timed {
    fn calls_per_second(&self) -> u32 {
        self.host.calls_per_second()
    }

    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError> {
        self.host.open(spec)
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        self.note(Step::ReloadAsked);
        let answer = self.host.reload();
        self.note(Step::ReloadAnswered(match &answer {
            Ok(Reload::Applied { generation }) => format!("applied {generation}"),
            Ok(Reload::Unchanged) => "unchanged".to_owned(),
            Ok(Reload::Failed { .. }) => "failed".to_owned(),
            Err(e) => format!("refused: {e}"),
        }));
        answer
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        self.host.alias(module, dir)
    }

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        self.host.resize(size)
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        self.note(Step::PictureAsked);
        let picture = self.host.screenshot(max_side);
        self.note(Step::PictureAnswered(
            picture.as_ref().map_or(0, |p| p.png.len()),
        ));
        picture
    }

    fn outline(&mut self) -> Result<String, PreviewError> {
        self.host.outline()
    }

    fn read(
        &mut self,
        id: &str,
        names: &[String],
    ) -> Result<serde_json::Map<String, serde_json::Value>, PreviewError> {
        self.host.read(id, names)
    }

    fn write(
        &mut self,
        id: &str,
        name: &str,
        value: serde_json::Value,
    ) -> Result<(), PreviewError> {
        self.host.write(id, name, value)
    }

    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
        self.host.logs(since)
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        self.host.pick(x, y)
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        self.host.alive()
    }

    fn close(&mut self) {
        self.host.close();
    }
}

struct Window {
    stage: Stage,
    root: ObjectId,
    workspace: ObjectId,
    steps: Steps,
    /// The preview's log, which `NativeHost` keeps in the state directory.
    log: std::path::PathBuf,
    /// The document the edits write.
    document: std::path::PathBuf,
}

impl Window {
    /// Opens Studio on `project` until its first frame lists the documents.
    fn open(project: &Path, state_dir: &Path) -> anyhow::Result<Window> {
        let state = state_dir.to_owned();
        let steps = Steps::default();
        let host_steps = Arc::clone(&steps);
        let mut studio = Studio::open(project)?.with_previews(Arc::new(move |dir| {
            Box::new(Timed {
                host: NativeHost::new(dir, NativeHost::default_program(), &state),
                steps: Arc::clone(&host_steps),
            })
        }));
        let told = Arc::clone(&steps);
        studio.watch(Arc::new(move || {
            told.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((Instant::now(), Step::Told));
        }));
        let server = Arc::new(StudioServer::new(studio));
        let engine = ljos::engine();
        studio_blocks::register(&engine);
        let mut stage = Stage::new(&engine, WIDTH, HEIGHT);
        studio_blocks::serve(&engine, server);
        let source = ljos::include_qml!("shell.pack");
        // Studio's window with no shell of the person's: it shows its own.
        let root = ljos::create_root(&engine, source, "Window")?;
        stage.scene.set_root(&engine, root)?;
        stage.scene.set_accessibility_active(&engine, true);
        let workspace = find_workspace(&engine, root).ok_or(anyhow!("no Studio.Workspace"))?;
        let mut window = Window {
            stage,
            root,
            workspace,
            steps,
            log: state_dir.join("preview.log"),
            document: project.join("Main.qml"),
        };
        window.rest()?;
        window.node("Documents", "Main.qml")?;
        Ok(window)
    }

    fn rest(&mut self) -> anyhow::Result<()> {
        let mut driver = self.stage.drive();
        driver.settle()?;
        driver.finish_promised()?;
        Ok(())
    }

    fn tree(&self) -> AccessibleNode {
        ljos_drive::a11y::tree(&self.stage.scene, String::new())
    }

    fn node(&self, within: &str, name: &str) -> anyhow::Result<AccessibleNode> {
        self.node_with(within, name, |_| true)
    }

    fn node_with(
        &self,
        within: &str,
        name: &str,
        test: impl Fn(&AccessibleNode) -> bool,
    ) -> anyhow::Result<AccessibleNode> {
        let tree = self.tree();
        let container = find(&tree, &|n| n.name == within && n.role != "label")
            .ok_or_else(|| anyhow!("no {within:?} in the window"))?;
        find(container, &|n| n.name == name && n.key.is_some() && test(n))
            .cloned()
            .ok_or_else(|| anyhow!("no {name:?} in {within:?}"))
    }

    fn layer(&self, label: &str) -> anyhow::Result<AccessibleNode> {
        self.node_with("Layers", label, |n| n.role != "label")
    }

    fn inspector_field(&self, name: &str) -> anyhow::Result<AccessibleNode> {
        self.node_with("Inspector", name, |n| n.role == "textinput")
    }

    fn click(&mut self, node: &AccessibleNode) -> anyhow::Result<()> {
        let key = node.key.ok_or(anyhow!("the node has no key"))?;
        let mut driver = self.stage.drive();
        let at = driver
            .aim_at_node(key)
            .map_err(|why| anyhow!("{:?} is not clicked: {why:?}", node.name))?;
        driver.click(at, MouseButtons::LEFT, 1)?;
        self.rest()
    }

    fn enter(&mut self, node: &AccessibleNode, text: &str) -> anyhow::Result<()> {
        self.click(node)?;
        let mut driver = self.stage.drive();
        driver.chord(&Chord::parse("Ctrl+A").map_err(|e| anyhow!(e))?, 0)?;
        driver.commit(text)?;
        driver.chord(&Chord::parse("Return").map_err(|e| anyhow!(e))?, 0)?;
        self.rest()
    }

    /// `rounds` sets of `Colors.canvas` in the tokens panel, each timed
    /// from Enter until the frame draws the preview's reload, one per
    /// `pace`.
    fn token_edits(&mut self, rounds: usize, pace: Duration) -> anyhow::Result<Vec<Duration>> {
        let filter = self.node_with("Tokens", "Filter tokens", |n| n.role == "textinput")?;
        self.enter(&filter, "Colors.canvas")?;
        let mut edits = Vec::new();
        let mut last: Option<Instant> = None;
        for round in 0..rounds {
            if let Some(last) = last {
                std::thread::sleep(pace.saturating_sub(last.elapsed()));
            }
            let shown = self.picture()?;
            let field =
                self.node_with("Tokens", "Colors.canvas light", |n| n.role == "textinput")?;
            let started = Instant::now();
            last = Some(started);
            self.enter(&field, CANVASES[round % 2])?;
            let refused = self
                .stage
                .engine
                .read_by_name(self.workspace, "error")
                .to_js_string();
            if !refused.is_empty() {
                bail!("theme_set was refused: {refused}");
            }
            self.wait_for_picture(shown + 1)?;
            edits.push(started.elapsed());
        }
        Ok(edits)
    }

    /// The value the inspector's `field` shows.
    fn inspector_value(&self, field: &str) -> anyhow::Result<String> {
        let node = self.inspector_field(field)?;
        Ok(node.value.clone())
    }

    /// A drag of `moves` pointer moves over the canvas's Desktop frame,
    /// each timed from the event until the window is at rest.
    fn drag_frames(&mut self, moves: usize) -> anyhow::Result<Vec<Duration>> {
        let frame = self.node_with("Canvas", "Desktop", |n| n.role != "label")?;
        let key = frame.key.ok_or(anyhow!("the frame has no key"))?;
        let mut driver = self.stage.drive();
        let start = driver
            .aim_at_node(key)
            .map_err(|why| anyhow!("the frame is not pressed: {why:?}"))?;
        driver.press(start, MouseButtons::LEFT)?;
        let mut samples = Vec::with_capacity(moves);
        let mut at = start;
        for i in 0..moves {
            // Out and back along a diagonal, 4 px a move, as a hand drags.
            let leg = (i / 50) % 2;
            let by = if leg == 0 { -4.0 } else { 4.0 };
            at.x += by;
            at.y += by / 2.0;
            let started = Instant::now();
            driver.move_to(at)?;
            // The move only queues the pointer; its frame's work is done at rest.
            driver.settle()?;
            samples.push(started.elapsed());
        }
        driver.release(at, MouseButtons::LEFT)?;
        driver.settle()?;
        // A drag that moved nothing would time an idle window.
        let end = driver
            .aim_at_node(key)
            .map_err(|why| anyhow!("the frame is gone after the drag: {why:?}"))?;
        if end == start {
            bail!("the drag did not pan the canvas's frame");
        }
        Ok(samples)
    }

    /// [`Window::edits`] of `field` that alternate a step either side of
    /// `around`.
    fn edits_between(
        &mut self,
        field: &str,
        rounds: usize,
        around: f64,
        pace: Option<Duration>,
    ) -> anyhow::Result<Vec<Edit>> {
        self.edits_with(field, rounds, pace, |round| {
            format!("{}", around + if round % 2 == 0 { 1.0 } else { 0.0 })
        })
    }

    fn open_document(&mut self, doc: &str) -> anyhow::Result<()> {
        let row = self.node("Documents", doc)?;
        self.click(&row)?;
        let shown = self
            .stage
            .engine
            .read_by_name(self.workspace, "doc")
            .to_js_string();
        if shown.as_ref() as &str != doc {
            bail!("the window shows {shown:?}");
        }
        Ok(())
    }

    /// `rounds` edits of the inspector's `field`, from `first` up, each
    /// timed from Enter until the frame draws the reload; with `pace`, each
    /// starts that long after the one before.
    fn edits(
        &mut self,
        field: &str,
        rounds: usize,
        first: usize,
        pace: Option<Duration>,
    ) -> anyhow::Result<Vec<Edit>> {
        self.edits_with(field, rounds, pace, |round| format!("{}", first + round))
    }

    fn edits_with(
        &mut self,
        field: &str,
        rounds: usize,
        pace: Option<Duration>,
        value: impl Fn(usize) -> String,
    ) -> anyhow::Result<Vec<Edit>> {
        let mut edits = Vec::new();
        let mut last: Option<Instant> = None;
        for round in 0..rounds {
            if let (Some(pace), Some(last)) = (pace, last) {
                std::thread::sleep(pace.saturating_sub(last.elapsed()));
            }
            let shown = self.picture()?;
            let field = self.inspector_field(field)?;
            self.steps
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
            let logged = std::fs::metadata(&self.log).map_or(0, |m| m.len());
            let wall = std::time::SystemTime::now();
            let started = Instant::now();
            last = Some(started);
            self.enter(&field, &value(round))?;
            let rested = started.elapsed();
            let looks = self.wait_for_picture(shown + 1)?;
            let took = started.elapsed();
            let steps = std::mem::take(
                &mut *self
                    .steps
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            let steps = steps
                .into_iter()
                .map(|(at, step)| (at.saturating_duration_since(started), step))
                .collect();
            let child = std::fs::read(&self.log)
                .map(|log| {
                    let new = log.get(logged as usize..).unwrap_or_default();
                    String::from_utf8_lossy(new)
                        .lines()
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let written = std::fs::metadata(&self.document).ok().map(|m| {
                use std::os::unix::fs::MetadataExt;
                let since = |secs: i64, nanos: i64| {
                    let at = std::time::UNIX_EPOCH
                        + Duration::new(secs.max(0) as u64, nanos.max(0) as u32);
                    at.duration_since(wall).unwrap_or_default()
                };
                (
                    since(m.mtime(), m.mtime_nsec()),
                    since(m.ctime(), m.ctime_nsec()),
                )
            });
            edits.push(Edit {
                took,
                steps,
                rested,
                looks,
                written,
                child,
            });
        }
        Ok(edits)
    }

    /// The preview generation the canvas's frame draws.
    fn picture(&self) -> anyhow::Result<u64> {
        ljos_drive::find(&self.stage.engine, &[self.root], "frame Desktop")
            .map_err(|_| anyhow!("the canvas has no Desktop frame"))?;
        let preview = self.stage.engine.read_by_name(self.workspace, "preview");
        let preview = <serde_json::Value as QmlValue>::from_value(&preview).unwrap_or_default();
        Ok(preview["picture"].as_u64().unwrap_or(0))
    }

    /// That the frame draws `generation` once the window is at rest: the
    /// workspace's wait for the preview's reload and its picture's fetch are
    /// promised work, which resting waits for. An edit past the preview's
    /// reload budget is shown when the budget next allows, which nothing
    /// promised the window; the harness looks again until then (the window
    /// itself is told, not polled).
    /// Answers how many times it looked again.
    fn wait_for_picture(&mut self, generation: u64) -> anyhow::Result<usize> {
        let deadline = Instant::now() + CATCH_UP;
        let mut looks = 0;
        loop {
            self.rest()?;
            let shown = self.picture()?;
            if shown >= generation {
                return Ok(looks);
            }
            looks += 1;
            if Instant::now() >= deadline {
                bail!("the frame draws generation {shown}, not {generation}");
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn find<'n>(
    node: &'n AccessibleNode,
    test: &dyn Fn(&AccessibleNode) -> bool,
) -> Option<&'n AccessibleNode> {
    if test(node) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, test))
}

fn find_workspace(engine: &Engine, root: ObjectId) -> Option<ObjectId> {
    let mut stack = vec![root];
    while let Some(obj) = stack.pop() {
        if engine.with(obj, |_: &studio_blocks::Workspace| ()).is_ok() {
            return Some(obj);
        }
        stack.extend(engine.children(obj).into_iter().rev());
    }
    None
}
