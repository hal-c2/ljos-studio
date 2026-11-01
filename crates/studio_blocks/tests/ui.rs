//! Studio's window scenarios (`features/ui/`): a person drives Studio's own
//! shell (`shell/`), with Studio's blocks registered, in a window without a
//! platform on a test clock. Steps find controls as an assistive technology
//! does, by role and name, and click, type and press keys as a person does;
//! the oracle is the file text, as for the MCP scenarios.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use cucumber::{World, gherkin::Step, given, then, when};
use ljos::item::{DrawOp, MouseButtons, PointerEvent, WheelEvent, kurbo::Point};
use ljos::prelude::*;
use ljos::testing::AccessibleNode;
use ljos_dev::{DevSession, Host, Report};
use ljos_drive::{Chord, DriveError, Stage, Wheel};
use ljos_mcp_core::{CallCtx, Cap, CapSet};
use ljos_runtime::SystemClock;
use serde_json::Value as Json;
use studio_core::preview::{
    Click, Declared, LogLine, Picture, Placed, PreviewError, PreviewHost, PreviewSpec, Reload,
    RestWatch, Size,
};
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

/// The window's size: a laptop's screen, which a desktop frame at full size
/// does not fit beside the panels; the canvas scrolls.
const WIDTH: f64 = 1440.0;
const HEIGHT: f64 = 900.0;

#[derive(World)]
#[world(init = Self::new)]
struct UiWorld {
    /// Where a frame was before the wheel turned.
    frame_before: Option<ljos::item::kurbo::Rect>,
    /// Where the pointer was for a zoom.
    pointer_was: Option<Point>,
    /// A workspace a scenario expects to go.
    remembered: Option<ObjectId>,
    /// The threads following a picture before the one a scenario starts, and
    /// that one once it is told.
    followers_before: Vec<String>,
    follower: Option<String>,
    /// Where the outline was shown, bare of a ghost, in the frames of a flick.
    outlines: Vec<(f64, f64)>,
    dir: tempfile::TempDir,
    /// Where previews keep their token files and logs, apart from the
    /// project the preview watches.
    state: tempfile::TempDir,
    window: Option<Window>,
    /// How many different pictures the frame had drawn when last looked at.
    looked: u64,
    asked: u64,
    /// The previews say nothing of where their objects are, as a web host.
    no_places: bool,
    picture_ms: Arc<AtomicU64>,
    /// Where a step left the hand holding the canvas.
    held: Option<Point>,
}

/// A preview that answers everything but `placed`, whose default refuses.
struct NoPlaces(NativeHost);

/// A preview that spends what a step says (in ms) over each picture.
struct SlowPictures(NativeHost, Arc<AtomicU64>);

impl PreviewHost for SlowPictures {
    fn scene(
        &mut self,
        at: &Declared,
    ) -> Result<Option<studio_core::scene3d::Scene>, PreviewError> {
        self.0.scene(at)
    }

    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError> {
        self.0.open(spec)
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        self.0.reload()
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        self.0.alias(module, dir)
    }

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        self.0.resize(size)
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        std::thread::sleep(Duration::from_millis(self.1.load(Ordering::SeqCst)));
        self.0.screenshot(max_side)
    }

    fn outline(&mut self) -> Result<String, PreviewError> {
        self.0.outline()
    }

    fn rest_watch(&self) -> Option<Box<dyn RestWatch>> {
        self.0.rest_watch()
    }

    fn read(
        &mut self,
        id: &str,
        names: &[String],
    ) -> Result<serde_json::Map<String, Json>, PreviewError> {
        self.0.read(id, names)
    }

    fn write(&mut self, id: &str, name: &str, value: Json) -> Result<(), PreviewError> {
        self.0.write(id, name, value)
    }

    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
        self.0.logs(since)
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        self.0.pick(x, y)
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        self.0.alive()
    }

    fn close(&mut self) {
        self.0.close();
    }

    fn calls_per_second(&self) -> u32 {
        self.0.calls_per_second()
    }

    fn placed(&mut self, ids: &[String], types: &[String]) -> Result<Vec<Placed>, PreviewError> {
        self.0.placed(ids, types)
    }

    fn click(&mut self, at: &Click) -> Result<(), PreviewError> {
        self.0.click(at)
    }
}

impl PreviewHost for NoPlaces {
    fn scene(
        &mut self,
        at: &Declared,
    ) -> Result<Option<studio_core::scene3d::Scene>, PreviewError> {
        self.0.scene(at)
    }

    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError> {
        self.0.open(spec)
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        self.0.reload()
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        self.0.alias(module, dir)
    }

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        self.0.resize(size)
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        self.0.screenshot(max_side)
    }

    fn outline(&mut self) -> Result<String, PreviewError> {
        self.0.outline()
    }

    fn rest_watch(&self) -> Option<Box<dyn RestWatch>> {
        self.0.rest_watch()
    }

    fn read(
        &mut self,
        id: &str,
        names: &[String],
    ) -> Result<serde_json::Map<String, Json>, PreviewError> {
        self.0.read(id, names)
    }

    fn write(&mut self, id: &str, name: &str, value: Json) -> Result<(), PreviewError> {
        self.0.write(id, name, value)
    }

    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
        self.0.logs(since)
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        self.0.pick(x, y)
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        self.0.alive()
    }

    fn close(&mut self) {
        self.0.close();
    }
}

struct Window {
    overflowed: Rc<Cell<Option<String>>>,
    /// Dropped first: the watcher's thread ends before the engine goes.
    watch: Option<Watch>,
    stage: Stage,
    root: ObjectId,
    workspace: ObjectId,
    server: Arc<StudioServer>,
    /// What the window did while nobody touched it.
    idle: Option<Idle>,
}

struct Idle {
    wake_before: Option<Time>,
    wake_after: Option<Time>,
    shown: u64,
}

impl std::fmt::Debug for UiWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiWorld")
            .field("dir", &self.dir.path())
            .finish_non_exhaustive()
    }
}

type StepResult = Result<(), String>;

fn drive_error(e: DriveError) -> String {
    e.to_string()
}

impl UiWorld {
    fn new() -> Self {
        Self {
            frame_before: None,
            pointer_was: None,
            remembered: None,
            followers_before: Vec::new(),
            follower: None,
            outlines: Vec::new(),
            dir: tempfile::tempdir().expect("a temporary directory"),
            state: tempfile::tempdir().expect("a temporary directory"),
            window: None,
            looked: 0,
            asked: 0,
            no_places: false,
            picture_ms: Arc::default(),
            held: None,
        }
    }

    fn window(&mut self) -> Result<&mut Window, String> {
        self.window
            .as_mut()
            .ok_or_else(|| "Studio's window is not open".to_owned())
    }

    /// Studio's window on the shell Studio ships, compiled once.
    fn open(&mut self) -> StepResult {
        self.open_with(|engine| {
            let source = ljos::dev::compile_dir_with(shell().as_ref(), engine)
                .map_err(|e| format!("the shell does not compile: {e}"))?;
            if let Some((key, why)) = source.failures().next() {
                return Err(format!("the shell's {key} does not compile: {why}"));
            }
            let root = ljos::create_root(engine, source, "Window")
                .map_err(|e| format!("the shell is not made: {e}"))?;
            Ok((root, None))
        })
    }

    /// Studio's window as `studio --shell` runs it on the project's
    /// directory: the person's shell there and Studio's beneath it,
    /// watched with the theme file beside the person's `Shell.qml`.
    fn open_watched(&mut self) -> StepResult {
        // As the watcher names what changed, so a save of the theme file is
        // known for it.
        let dir = self.dir.path().canonicalize().map_err(|e| e.to_string())?;
        let builtin = self
            .state
            .path()
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("shell");
        studio_blocks::shell::write(&builtin).map_err(|e| e.to_string())?;
        let user_shell =
            studio_blocks::shell::url(&dir.join("Shell.qml")).map_err(|e| e.to_string())?;
        self.open_with(move |engine| {
            let (tx, woke) = mpsc::channel();
            let session = DevSession::open_roots_themed(
                engine,
                &[&dir, &builtin],
                &["Window"],
                vec![("userShell".to_owned(), user_shell.as_str().into())],
                ljos_dev::Aliases::default(),
                Some(&dir.join("theme.json")),
                ljos::dev::EngineOracle::from_engine(engine),
                Arc::new(move || {
                    let _ = tx.send(());
                }),
            )
            .map_err(|e| format!("the shell cannot be watched: {e}"))?;
            // As `studio` does once the window is made: a theme that did
            // not load as it opened sent no report.
            studio_blocks::reload_settled(engine);
            let root = session.root().ok_or("the watched shell made no window")?;
            Ok((
                root,
                Some(Watch {
                    session,
                    clock: SystemClock::new(),
                    woke,
                }),
            ))
        })
    }

    fn open_with(
        &mut self,
        make: impl FnOnce(&Engine) -> Result<(ObjectId, Option<Watch>), String>,
    ) -> StepResult {
        // A directory for the host to make, as the program names one: a
        // temporary directory itself is as open as the umask leaves it.
        let state = self.state.path().join("preview");
        let no_places = self.no_places;
        let picture_ms = Arc::clone(&self.picture_ms);
        let studio = Studio::open(self.dir.path())
            .map_err(|e| e.to_string())?
            .with_previews(Arc::new(move |dir| {
                let host = NativeHost::new(dir, NativeHost::default_program(), &state);
                if no_places {
                    Box::new(NoPlaces(host))
                } else {
                    Box::new(SlowPictures(host, Arc::clone(&picture_ms)))
                }
            }));
        let server = Arc::new(StudioServer::new(studio));
        let engine = ljos::engine();
        let overflowed = Rc::new(Cell::new(None));
        let seen = Rc::clone(&overflowed);
        engine.set_warning_handler(move |message| {
            if message.contains("Maximum call stack") {
                seen.set(Some(message.to_owned()));
            }
        });
        studio_blocks::register(&engine);
        let mut stage = Stage::new(&engine, WIDTH, HEIGHT);
        studio_blocks::serve(&engine, Arc::clone(&server));
        let (root, watch) = make(&engine)?;
        stage
            .scene
            .set_root(&engine, root)
            .map_err(|e| e.to_string())?;
        stage.scene.set_accessibility_active(&engine, true);
        let mut window = Window {
            overflowed,
            watch,
            stage,
            root,
            workspace: root,
            server,
            idle: None,
        };
        if window.loader_status()? == ShellStatus::Null {
            return Err(format!(
                "the window shows no shell: {}",
                window.loader_read("error").to_js_string()
            ));
        }
        window.workspace = window.find_workspace()?;
        window.rest()?;
        self.window = Some(window);
        Ok(())
    }
}

/// A `ShellLoader`'s `status`, as `Ljos.Blocks` numbers it.
#[derive(Debug, PartialEq, Eq)]
enum ShellStatus {
    Null,
    Ready,
    Fallback,
    Error,
}

/// The watched session behind a window `studio --shell` runs, pumped as
/// the shell's event loop pumps it: when the watcher wakes it.
struct Watch {
    session: DevSession,
    clock: SystemClock,
    woke: mpsc::Receiver<()>,
}

/// Tells the window what `reports` say of the person's shell, as
/// `studio`'s reload hook does; whether one of them answers a save.
fn told(engine: &Engine, reports: &[Report]) -> bool {
    let mut answered = false;
    for report in reports {
        match report {
            Report::Failed { .. }
            | Report::Reloaded(_)
            | Report::Cleared
            | Report::Theme { .. } => {
                studio_blocks::reload_settled(engine);
                answered = true;
            }
            _ => {}
        }
    }
    answered
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

/// How long a save may take to reach the window: the watcher's quiet
/// period and a build of the shell, on a loaded machine.
const PATIENCE: Duration = Duration::from_secs(30);
/// How long a step may wait for the window's answers on a loaded machine.
const REST_PATIENCE: Duration = Duration::from_secs(120);
/// How long a scenario may take before the run stops and names it: what a
/// step's own patience cannot reach, a call blocked on a lock.
const SCENARIO_PATIENCE: Duration = Duration::from_secs(600);

/// The crate's directory as cargo names it to the running test: the shared
/// compile cache may hand a binary built in another clone of the
/// repository, whose `env!` path is that clone's.
fn manifest_dir() -> String {
    std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned())
}

fn shell() -> String {
    format!("{}/../../shell", manifest_dir())
}

impl Window {
    fn engine(&self) -> &Engine {
        &self.stage.engine
    }

    /// The first `Studio.Workspace` of the shell the window shows, depth
    /// first.
    /// The workspace of the shell the window shows: one the loader gave up
    /// on may still be below the window until it is collected.
    fn find_workspace(&self) -> Result<ObjectId, String> {
        let engine = self.engine();
        let shown = self.loader_read("item").as_object().unwrap_or(self.root);
        let mut stack = vec![shown];
        while let Some(obj) = stack.pop() {
            if engine.with(obj, |_: &studio_blocks::Workspace| ()).is_ok() {
                return Ok(obj);
            }
            stack.extend(engine.children(obj).into_iter().rev());
        }
        Err("the shell binds no Studio.Workspace".to_owned())
    }

    fn loader_read(&self, name: &str) -> Value {
        ljos_drive::find_below(self.engine(), self.root, "shellLoader")
            .map_or(Value::Null, |loader| {
                self.engine().read_by_name(loader, name)
            })
    }

    fn loader_status(&self) -> Result<ShellStatus, String> {
        if ljos_drive::find_below(self.engine(), self.root, "shellLoader").is_none() {
            return Err("the window has no shell loader".to_owned());
        }
        let status = self.loader_read("status");
        match status.as_number() {
            Some(0.0) => Ok(ShellStatus::Null),
            Some(1.0) => Ok(ShellStatus::Ready),
            Some(2.0) => Ok(ShellStatus::Fallback),
            Some(3.0) => Ok(ShellStatus::Error),
            _ => Err(format!("the shell loader's status is {status:?}")),
        }
    }

    /// What the window's banner says, empty when it is hidden.
    fn banner(&self) -> Result<String, String> {
        let banner = ljos_drive::find_below(self.engine(), self.root, "shellBanner")
            .ok_or("the window has no shell banner")?;
        if self.engine().read_by_name(banner, "visible") != Value::Bool(true) {
            return Ok(String::new());
        }
        Ok(self
            .engine()
            .read_by_name(banner, "text")
            .to_js_string()
            .to_string())
    }

    /// Waits for the watcher to see the last save and pumps the session
    /// until it has applied it, or said why not, as Studio's event loop
    /// does; then tells the window, as `studio`'s reload hook does, and
    /// lets what the reload made settle.
    fn take_change(&mut self) -> StepResult {
        let Self { watch, stage, .. } = self;
        let watch = watch
            .as_mut()
            .ok_or("Studio's window does not watch a shell")?;
        let mut seen = Vec::new();
        loop {
            watch.woke.recv_timeout(PATIENCE).map_err(|_| {
                format!("the window took no change in {PATIENCE:?}; it reported {seen:?}")
            })?;
            let reports = watch.session.pump(
                &stage.engine,
                &watch.clock,
                &mut SceneHost(&mut stage.scene),
            );
            let taken = told(&stage.engine, &reports);
            seen.extend(reports);
            if taken {
                break;
            }
        }
        self.root = self.stage.scene.root().ok_or("the window shows nothing")?;
        self.rest()?;
        // The frame after a reload has run: the session matches what it
        // made late, a Loader's item among them.
        let Self { watch, stage, .. } = self;
        if let Some(watch) = watch {
            let reports = watch.session.pump(
                &stage.engine,
                &watch.clock,
                &mut SceneHost(&mut stage.scene),
            );
            told(&stage.engine, &reports);
        }
        self.rest()?;
        // A reload may make the shell again, and its workspace with it.
        if self.loader_status()? != ShellStatus::Null {
            self.workspace = self.find_workspace()?;
        }
        Ok(())
    }

    /// Lets everything the last gesture started finish: its tool calls,
    /// which run off the UI thread, what they publish, and the pictures
    /// they fetch, then the frames that show it.
    fn rest(&mut self) -> StepResult {
        let mut patience = self.patience();
        let mut driver = self.stage.drive().serving(Some(&mut patience));
        driver.settle().map_err(drive_error)?;
        driver.finish_promised().map_err(drive_error)?;
        // A binding that runs out of stack keeps its old value and says so
        // only in a warning: the window looks stale for no visible reason.
        match self.overflowed.take() {
            Some(warning) => Err(format!("the window ran out of JS stack: {warning}")),
            None => Ok(()),
        }
    }

    /// Lets `ms` pass on the test clock, as patient as `rest`.
    fn advance(&mut self, ms: u64) -> StepResult {
        let mut patience = self.patience();
        self.stage
            .drive()
            .serving(Some(&mut patience))
            .advance(ms)
            .map_err(drive_error)
    }

    /// Told every few milliseconds while the driver waits on answers. The
    /// wait itself has no end while answers keep coming, so this ends it,
    /// saying what it was waiting for; the step fails on the panic.
    fn patience(&self) -> impl FnMut() + use<> {
        let platform = Platform::of(&self.stage.engine);
        let started = std::time::Instant::now();
        move || {
            if started.elapsed() > REST_PATIENCE {
                panic!(
                    "the window did not come to rest in {REST_PATIENCE:?}: \
                     still waiting on {} platform answer(s)",
                    platform.pending()
                );
            }
        }
    }

    /// The answer to a gesture's call, as the worker's arrives: for one that
    /// was on its way when the person did something else.
    fn answer(&mut self, tool: &str, answer: Json, asked: Json) -> StepResult {
        let workspace = self.workspace;
        let engine = &mut self.stage.engine;
        studio_blocks::Workspace::emit_answered(
            &Cx::new(engine, workspace),
            tool.to_owned(),
            answer,
            asked,
        )
        .map_err(|e| format!("{e:?}"))
    }

    fn tree(&self) -> AccessibleNode {
        ljos_drive::a11y::tree(&self.stage.scene, String::new())
    }

    fn read(&self, name: &str) -> Value {
        self.engine().read_by_name(self.workspace, name)
    }

    fn read_json(&self, name: &str) -> Json {
        <Json as QmlValue>::from_value(&self.read(name)).unwrap_or_default()
    }

    /// Clicks `row` of the list named `list` where it shows: a list
    /// clips the rows past its ends, and the driver aims at a node's
    /// middle whatever clips it, so a row half past the end would give
    /// the click to what lies below the list. A row past an end is
    /// scrolled to first, a wheel notch at a time over the list, as a
    /// person does.
    fn click_row(&mut self, list: &str, row: &AccessibleNode) -> StepResult {
        let key = row.key.ok_or("the row has no key")?;
        let shown = self
            .bounds(&|role, name| role == "list" && name == list)
            .ok_or_else(|| format!("no list {list:?} in the window"))?;
        let at = |w: &Self| {
            let mut at = None;
            w.stage.scene.accessibility_tree().walk(|k, _, d| {
                if k == key {
                    let b = d.bounds;
                    at = Some(ljos::item::kurbo::Rect::new(b.x0, b.y0, b.x1, b.y1));
                }
            });
            at.ok_or_else(|| format!("{:?} is not in {list:?}", row.name))
        };
        let mut now = at(self)?;
        while now.y0 < shown.y0 || now.y1 > shown.y1 {
            // A positive notch turns the wheel away, which shows what is
            // above.
            let notch = if now.y1 > shown.y1 { -1.0 } else { 1.0 };
            self.stage
                .drive()
                .wheel(shown.center(), Wheel::Notches(0.0, notch))
                .map_err(drive_error)?;
            // A wheel notch eases the list over time; the person waits for
            // it to stop, on the test clock.
            self.advance(1000)?;
            let then = at(self)?;
            if then.y0 == now.y0 {
                break;
            }
            now = then;
        }
        let seen = now.intersect(shown);
        if seen.area() <= 0.0 {
            return Err(format!(
                "{:?} at {now:?} is out of the view of {list:?}, {shown:?}, and the wheel does not bring it in",
                row.name
            ));
        }
        self.stage
            .drive()
            .click(seen.center(), MouseButtons::LEFT, 1)
            .map_err(drive_error)?;
        self.rest()
    }

    /// Brings a panel behind a tab to the front, as a person has to before
    /// using it: a panel not shown is not in the tree.
    fn open_panel(&mut self, panel: &str) -> StepResult {
        let tab = match panel {
            "Palette" => "Insert tab",
            "Layers" | "Documents" => "Layers tab",
            "Inspector" => "Design tab",
            "Tokens" => "Tokens tab",
            "Flow" => "Flow tab",
            _ => return Ok(()),
        };
        let tree = self.tree();
        if find(&tree, &|n| n.name == panel && n.role != "label").is_some() {
            return Ok(());
        }
        let Some(node) = find(&tree, &|n| n.name == tab && n.key.is_some()).cloned() else {
            return Ok(());
        };
        self.click_node(&node)
    }

    /// The node named `name` inside the one named `within`.
    fn node(
        &mut self,
        within: &str,
        role: Option<&str>,
        name: &str,
    ) -> Result<AccessibleNode, String> {
        self.open_panel(within)?;
        let tree = self.tree();
        let container = find(&tree, &|n| n.name == within && n.role != "label")
            .ok_or_else(|| format!("no {within:?} in the window"))?;
        find(container, &|n| {
            n.name == name && role.is_none_or(|r| n.role == r) && n.key.is_some()
        })
        .cloned()
        .ok_or_else(|| {
            format!(
                "no {}{name:?} in {within:?}; it holds {}",
                role.map(|r| format!("{r} ")).unwrap_or_default(),
                names(container)
            )
        })
    }

    /// Clicks the part of `node` the window shows: a text area as tall as
    /// its text reaches past the window's edge, and the driver aims at a
    /// node's middle wherever that is.
    fn click_shown(&mut self, node: &AccessibleNode) -> StepResult {
        let key = node.key.ok_or("the node has no key")?;
        let mut at = None;
        self.stage.scene.accessibility_tree().walk(|k, _, d| {
            if k == key {
                let b = d.bounds;
                at = Some(ljos::item::kurbo::Rect::new(b.x0, b.y0, b.x1, b.y1));
            }
        });
        let at = at.ok_or_else(|| format!("{:?} is not in the window", node.name))?;
        let seen = at.intersect(ljos::item::kurbo::Rect::new(0.0, 0.0, WIDTH, HEIGHT));
        if seen.area() <= 0.0 {
            return Err(format!("{:?} at {at:?} is out of the window", node.name));
        }
        self.stage
            .drive()
            .click(seen.center(), MouseButtons::LEFT, 1)
            .map_err(drive_error)?;
        self.rest()
    }

    fn click_node(&mut self, node: &AccessibleNode) -> StepResult {
        let key = node.key.ok_or("the node has no key")?;
        let mut driver = self.stage.drive();
        let at = driver
            .aim_at_node(key)
            .map_err(|why| format!("{:?} {:?} is not clicked: {why:?}", node.role, node.name))?;
        driver
            .click(at, MouseButtons::LEFT, 1)
            .map_err(drive_error)?;
        self.rest()
    }

    /// Focuses a field, puts `text` in it in place of what it held, as a
    /// person selecting all and typing does, and presses Enter.
    fn enter(&mut self, node: &AccessibleNode, text: &str) -> StepResult {
        self.click_node(node)?;
        let mut driver = self.stage.drive();
        driver
            .chord(&Chord::parse("Ctrl+A")?, 0)
            .map_err(drive_error)?;
        driver.commit(text).map_err(drive_error)?;
        driver
            .chord(&Chord::parse("Return")?, 0)
            .map_err(drive_error)?;
        self.rest()
    }

    fn layers(&self) -> Vec<String> {
        self.read_json("layers")
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|r| r["label"].as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The rows the layers panel shows, in order.
    fn shown_layers(&mut self) -> Result<Vec<AccessibleNode>, String> {
        self.open_panel("Layers")?;
        let tree = self.tree();
        let list = find(&tree, &|n| n.name == "Layers" && n.role == "list")
            .ok_or("no layers list in the window")?;
        let mut rows = Vec::new();
        collect(list, &mut |n| {
            // The row's eye and lock and its name field are not rows.
            let control = ["Hide ", "Show ", "Lock ", "Unlock ", "Name of "]
                .iter()
                .any(|p| n.name.starts_with(p));
            if n.key.is_some()
                && !control
                && n.name != "Layers"
                && n.role != "label"
                && n.role != "statictext"
            {
                rows.push(n.clone());
            }
        });
        Ok(rows)
    }

    fn code(&self) -> Result<AccessibleNode, String> {
        let tree = self.tree();
        find(&tree, &|n| n.object_name == "code")
            .cloned()
            .ok_or_else(|| "no code view in the window".to_owned())
    }

    /// Where the first node `test` takes is in the window.
    fn bounds(&self, test: &dyn Fn(&str, &str) -> bool) -> Option<ljos::item::kurbo::Rect> {
        let mut found = None;
        self.stage.scene.accessibility_tree().walk(|_, _, d| {
            if found.is_none() && test(&ljos_a11y::role_word(d.role), &d.name) {
                let b = d.bounds;
                found = Some(ljos::item::kurbo::Rect::new(b.x0, b.y0, b.x1, b.y1));
            }
        });
        found
    }

    fn frame(&self, target: &str) -> Result<ObjectId, String> {
        ljos_drive::find(self.engine(), &[self.root], &format!("frame {target}")).map_err(|_| {
            format!(
                "the canvas has no frame {target:?}; the preview is {}, previews are {}, \
                     the last refusal {:?}",
                self.read_json("preview"),
                if self.read("previews") == Value::Bool(true) {
                    "on"
                } else {
                    "off"
                },
                self.read("error").to_js_string()
            )
        })
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

fn collect(node: &AccessibleNode, f: &mut dyn FnMut(&AccessibleNode)) {
    for c in &node.children {
        f(c);
        collect(c, f);
    }
}

fn names(node: &AccessibleNode) -> String {
    let mut all = Vec::new();
    collect(node, &mut |n| {
        if !n.name.is_empty() {
            all.push(format!("{} {:?}", n.role, n.name));
        }
    });
    all.join(", ")
}

fn list(text: &str) -> Vec<String> {
    text.split(',').map(|s| s.trim().to_owned()).collect()
}

fn docstring(step: &Step) -> Result<String, String> {
    let text = step
        .docstring
        .as_deref()
        .ok_or_else(|| "this step needs a doc string".to_owned())?;
    // The doc string's content type (`"""qml`) arrives as its first line.
    let text = match text.split_once('\n') {
        Some((first, rest)) if matches!(first.trim(), "qml" | "json" | "text" | "") => rest,
        _ => text,
    };
    Ok(dedent(text))
}

/// The doc string without the indentation its lines share: the runner keeps
/// the scenario's own.
fn dedent(text: &str) -> String {
    let indent = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    text.lines()
        .map(|l| l.get(indent..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A file as a doc string writes it: its lines, each ending in a newline.
fn file_text(step: &Step) -> Result<String, String> {
    Ok(format!("{}\n", docstring(step)?))
}

/// A Gherkin string's escapes: `\"` and `\n`.
fn unescape(text: &str) -> String {
    text.replace("\\n", "\n").replace("\\\"", "\"")
}

// ------------------------------------------------------------------ given

#[given(regex = r#"^the file "([^"]+)":$"#)]
fn the_file(w: &mut UiWorld, name: String, step: &Step) -> StepResult {
    let path = w.dir.path().join(&name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Whole or not at all: `fs::write` truncates first, and the shell's watcher
    // can wake in between and read a file with nothing in it. The new text
    // waits outside the project, where no watcher sees it.
    let scratch = w.dir.path().parent().ok_or("the project has no parent")?;
    let mut text = tempfile::NamedTempFile::new_in(scratch).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut text, file_text(step)?.as_bytes()).map_err(|e| e.to_string())?;
    text.persist(path).map(drop).map_err(|e| e.to_string())
}

#[given(regex = r"^(\d+) more documents$")]
fn more_documents(w: &mut UiWorld, count: usize) -> StepResult {
    for n in 0..count {
        let path = w.dir.path().join(format!("More{n}.qml"));
        std::fs::write(path, "Item {}\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[given("Studio's window shows the project")]
fn shows_the_project(w: &mut UiWorld) -> StepResult {
    w.open()
}

// ------------------------------------------------------------------- when

#[given(regex = r#"^the person has opened "([^"]+)" in the documents list$"#)]
#[when(regex = r#"^the person opens "([^"]+)" in the documents list$"#)]
fn opens(w: &mut UiWorld, doc: String) -> StepResult {
    let window = w.window()?;
    let row = window.node("Documents", None, &doc)?;
    window.click_row("Documents", &row)?;
    (window.read("doc").to_js_string().as_ref() as &str == doc)
        .then_some(())
        .ok_or_else(|| {
            format!(
                "the window shows {:?}; the last refusal {:?}",
                window.read("doc").to_js_string(),
                window.read("error").to_js_string()
            )
        })
}

#[given(regex = r#"^the person has selected "([^"]+)" in the layers panel$"#)]
#[when(regex = r#"^the person selects "([^"]+)" in the layers panel$"#)]
fn selects_layer(w: &mut UiWorld, label: String) -> StepResult {
    let window = w.window()?;
    let row = window
        .shown_layers()?
        .into_iter()
        .find(|n| n.name == label)
        .ok_or_else(|| {
            format!(
                "the layers panel lists {:?}; the window shows {}",
                window.layers(),
                names(&window.tree())
            )
        })?;
    window.click_node(&row)?;
    let selected = window.read("selected").to_js_string().to_string();
    let shown = window.read_json("layers").as_array().and_then(|rows| {
        rows.iter()
            .find(|r| r["node"] == selected.as_str())
            .cloned()
    });
    match shown {
        Some(row) if row["label"] == label.as_str() => Ok(()),
        _ => Err(format!(
            "{label:?} is not selected: {:?}",
            window.read("error").to_js_string()
        )),
    }
}

#[given(regex = r#"^the person has set "([^"]+)" to "(.*)" in the inspector$"#)]
#[when(regex = r#"^the person sets "([^"]+)" to "(.*)" in the inspector$"#)]
fn sets(w: &mut UiWorld, name: String, value: String) -> StepResult {
    let value = unescape(&value);
    let window = w.window()?;
    match window.node("Inspector", Some("textinput"), &name) {
        Ok(field) => window.enter(&field, &value),
        Err(_) => {
            let field = window.node("Inspector", Some("textinput"), "New property")?;
            window.enter(&field, &name)?;
            let field = window.node("Inspector", Some("textinput"), "New value")?;
            window.enter(&field, &value)
        }
    }
}

#[when(regex = r#"^the person enters "([^"]+)" as the new property in the inspector$"#)]
fn enters_new_property(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), "New property")?;
    window.enter(&field, &name)
}

#[then(regex = r#"^the inspector offers the field "([^"]+)"$"#)]
fn offers_field(w: &mut UiWorld, name: String) -> StepResult {
    w.window()?
        .node("Inspector", Some("textinput"), &name)
        .map(|_| ())
}

#[then(regex = r#"^the inspector does not offer the field "([^"]+)"$"#)]
fn no_field(w: &mut UiWorld, name: String) -> StepResult {
    match w.window()?.node("Inspector", Some("textinput"), &name) {
        Ok(_) => Err(format!("the inspector offers {name:?}")),
        Err(_) => Ok(()),
    }
}

#[when(regex = r#"^the person binds "([^"]+)" to "(.*)" in the inspector$"#)]
fn binds(w: &mut UiWorld, name: String, expression: String) -> StepResult {
    let expression = unescape(&expression);
    let window = w.window()?;
    let button = match window.node("Inspector", Some("textinput"), &name) {
        Ok(field) => {
            // Typed without Enter, which would set it as a literal.
            window.click_node(&field)?;
            let mut driver = window.stage.drive();
            driver
                .chord(&Chord::parse("Ctrl+A")?, 0)
                .map_err(drive_error)?;
            driver.commit(&expression).map_err(drive_error)?;
            window.rest()?;
            window.node("Inspector", None, &format!("Bind {name}"))?
        }
        Err(_) => {
            let field = window.node("Inspector", Some("textinput"), "New property")?;
            window.click_node(&field)?;
            window.stage.drive().commit(&name).map_err(drive_error)?;
            let field = window.node("Inspector", Some("textinput"), "New value")?;
            window.click_node(&field)?;
            window
                .stage
                .drive()
                .commit(&expression)
                .map_err(drive_error)?;
            window.rest()?;
            window.node("Inspector", None, "Bind new property")?
        }
    };
    window.click_node(&button)
}

#[given(regex = r#"^the person has placed "([^"]+)" from the palette$"#)]
#[when(regex = r#"^the person places "([^"]+)" from the palette$"#)]
fn places(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let button = window.node("Palette", None, &format!("Place {name}"))?;
    window.click_node(&button)
}

#[when(regex = r#"^the person imports "([^"]+)" in the palette$"#)]
fn imports_asset(w: &mut UiWorld, path: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Palette", Some("textinput"), "Path of a file to import")?;
    window.click_shown(&field)?;
    window.stage.drive().commit(&path).map_err(drive_error)?;
    window.rest()?;
    let button = window.node("Palette", None, "Import asset")?;
    window.click_node(&button)
}

#[when(regex = r#"^the person makes a component "([^"]+)" of the selection$"#)]
fn makes_component(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), "Component name")?;
    window.click_shown(&field)?;
    window.stage.drive().commit(&name).map_err(drive_error)?;
    window.rest()?;
    let button = window.node("Inspector", None, "Make component")?;
    window.click_node(&button)
}

#[given(regex = r#"^the person has searched the palette for "([^"]*)"$"#)]
#[when(regex = r#"^the person searches the palette for "([^"]*)"$"#)]
fn searches_palette(w: &mut UiWorld, text: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Palette", Some("textinput"), "Search the palette")?;
    window.click_shown(&field)?;
    for chord in ["Ctrl+A", "Delete"] {
        window
            .stage
            .drive()
            .chord(&Chord::parse(chord)?, 0)
            .map_err(drive_error)?;
    }
    if !text.is_empty() {
        window.stage.drive().commit(&text).map_err(drive_error)?;
    }
    window.rest()
}

#[then(regex = r#"^the palette (offers|does not offer) "([^"]+)"$"#)]
fn palette_offers(w: &mut UiWorld, how: String, name: String) -> StepResult {
    let window = w.window()?;
    let found = window
        .node("Palette", None, &format!("Place {name}"))
        .is_ok();
    if found != (how == "offers") {
        return Err(format!("the palette {how} {name}, and found it: {found}"));
    }
    Ok(())
}

#[then(regex = r#"^the palette says "(.*)"$"#)]
fn palette_says(w: &mut UiWorld, text: String) -> StepResult {
    let text = text.replace("\\\"", "\"");
    w.window()?.node("Palette", None, &text).map(|_| ())
}

#[given(regex = r#"^the person has clicked the button "([^"]+)"$"#)]
#[when(regex = r#"^the person clicks the button "([^"]+)"$"#)]
fn clicks_button(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let button = find(&tree, &|n| {
        n.name == name && n.role == "button" && n.key.is_some()
    })
    .cloned()
    .ok_or_else(|| format!("no button {name:?}; the window holds {}", names(&tree)))?;
    window.click_node(&button)
}

#[when(regex = r#"^the pointer rests on the button "([^"]+)" until its tip has shown$"#)]
fn rests_on_button(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let button = find(&tree, &|n| {
        n.name == name && n.role == "button" && n.key.is_some()
    })
    .cloned()
    .ok_or_else(|| format!("no button {name:?}; the window holds {}", names(&tree)))?;
    let mut driver = window.stage.drive();
    let at = driver
        .aim_at_node(button.key.ok_or("the button has no key")?)
        .map_err(|why| format!("{name:?} is not reached: {why:?}"))?;
    driver.move_to(at).map_err(drive_error)?;
    // Longer than the tip's delay.
    window.advance(800)?;
    window.rest()
}

#[given(regex = r#"^the person has added the state "([^"]+)" in the inspector$"#)]
#[when(regex = r#"^the person adds the state "([^"]+)" in the inspector$"#)]
fn adds_state(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), "New state")?;
    window.enter(&field, &name)?;
    let shown = shown_states(window)?;
    shown.contains(&name).then_some(()).ok_or_else(|| {
        format!(
            "the inspector lists the states {shown:?}; the last refusal {:?}",
            window.read("error").to_js_string()
        )
    })
}

#[given(regex = r#"^the person has typed the comment "(.*)" in the inspector$"#)]
fn types_comment(w: &mut UiWorld, text: String) -> StepResult {
    let text = unescape(&text);
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), "New comment")?;
    window.click_node(&field)?;
    window.stage.drive().commit(&text).map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person adds the comment "(.*)" in the inspector$"#)]
fn adds_comment(w: &mut UiWorld, text: String) -> StepResult {
    let text = unescape(&text);
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), "New comment")?;
    window.enter(&field, &text)
}

#[given(regex = r#"^the person has picked the state "([^"]+)" in the inspector$"#)]
#[when(regex = r#"^the person picks the state "([^"]+)" in the inspector$"#)]
fn picks_state(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let button = window.node("Inspector", Some("button"), &format!("State {name}"))?;
    window.click_node(&button)?;
    // Picking shows the state's condition, which only a picked state has.
    window
        .node("Inspector", Some("textinput"), "When")
        .map(drop)
}

#[when(regex = r#"^the person renames the state "([^"]+)" to "([^"]+)" in the inspector$"#)]
fn renames_state(w: &mut UiWorld, name: String, to: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), &format!("Name of {name}"))?;
    window.enter(&field, &to)
}

#[when(regex = r#"^the person changes "([^"]+)" of "([^"]+)" to "(.*)" in the state$"#)]
fn changes_in_state(w: &mut UiWorld, name: String, target: String, value: String) -> StepResult {
    let value = unescape(&value);
    let window = w.window()?;
    for (field, text) in [
        ("Change target", target.as_str()),
        ("Change property", name.as_str()),
        ("Change value", value.as_str()),
    ] {
        let field = window.node("Inspector", Some("textinput"), field)?;
        window.enter(&field, text)?;
    }
    Ok(())
}

#[then(regex = r#"^the inspector lists the states "([^"]*)"$"#)]
fn lists_states(w: &mut UiWorld, names: String) -> StepResult {
    let window = w.window()?;
    let want: Vec<String> = if names.is_empty() {
        Vec::new()
    } else {
        list(&names)
    };
    let shown = shown_states(window)?;
    (shown == want)
        .then_some(())
        .ok_or_else(|| format!("the inspector lists the states {shown:?}"))
}

/// The states the inspector shows a button for, in order.
fn shown_states(window: &mut Window) -> Result<Vec<String>, String> {
    window.open_panel("Inspector")?;
    let tree = window.tree();
    let inspector = find(&tree, &|n| n.name == "Inspector" && n.role != "label")
        .ok_or("no inspector in the window")?;
    let mut shown = Vec::new();
    collect(inspector, &mut |n| {
        if n.role == "button"
            && n.key.is_some()
            && let Some(name) = n.name.strip_prefix("State ")
        {
            shown.push(name.to_owned());
        }
    });
    Ok(shown)
}

/// The names in the panel `within` that start with `prefix`, without it,
/// in order.
fn shown_with(window: &mut Window, within: &str, prefix: &str) -> Result<Vec<String>, String> {
    window.open_panel(within)?;
    let tree = window.tree();
    let panel = find(&tree, &|n| n.name == within && n.role != "label")
        .ok_or_else(|| format!("no {within:?} in the window"))?;
    let mut shown = Vec::new();
    collect(panel, &mut |n| {
        if let Some(name) = n.name.strip_prefix(prefix) {
            shown.push(name.to_owned());
        }
    });
    Ok(shown)
}

fn listed(names: &str) -> Vec<String> {
    if names.is_empty() {
        Vec::new()
    } else {
        list(names)
    }
}

#[then(regex = r#"^the flow panel lists the states "([^"]*)"$"#)]
fn flow_lists_states(w: &mut UiWorld, names: String) -> StepResult {
    let shown = shown_with(w.window()?, "Flow", "Flow state ")?;
    (shown == listed(&names))
        .then_some(())
        .ok_or_else(|| format!("the flow panel lists the states {shown:?}"))
}

#[then(regex = r#"^the flow panel lists the flows "([^"]*)"$"#)]
fn flow_lists_flows(w: &mut UiWorld, names: String) -> StepResult {
    let window = w.window()?;
    let shown = shown_with(window, "Flow", "Flow edge ")?;
    (shown == listed(&names)).then_some(()).ok_or_else(|| {
        format!(
            "the flow panel lists the flows {shown:?}; the last refusal {:?}",
            window.read("error").to_js_string()
        )
    })
}

#[then(regex = r#"^the flow panel says the app is in the state "([^"]+)"$"#)]
fn flow_plays(w: &mut UiWorld, state: String) -> StepResult {
    let window = w.window()?;
    let shown = shown_with(window, "Flow", "Playing ")?;
    (shown == [state]).then_some(()).ok_or_else(|| {
        format!(
            "the flow panel says {shown:?}; the last refusal {:?}",
            window.read("error").to_js_string()
        )
    })
}

#[then(regex = r#"^the window shows a refusal that says "(.+)"$"#)]
fn refusal_says(w: &mut UiWorld, want: String) -> StepResult {
    let error = w.window()?.read("error").to_js_string();
    error
        .contains(&want)
        .then_some(())
        .ok_or_else(|| format!("the window shows the refusal {error:?}"))
}

#[then(regex = r#"^the inspector's button "([^"]+)" is disabled$"#)]
fn inspector_button_disabled(w: &mut UiWorld, name: String) -> StepResult {
    let button = w.window()?.node("Inspector", Some("button"), &name)?;
    button
        .states
        .iter()
        .any(|s| s == "disabled")
        .then_some(())
        .ok_or_else(|| format!("{name:?} is {:?}", button.states))
}

#[when(regex = r#"^the person types "(.*)" in the inspector's "([^"]+)" and leaves it$"#)]
fn types_in_row(w: &mut UiWorld, text: String, name: String) -> StepResult {
    let text = unescape(&text);
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), &name)?;
    window.click_node(&field)?;
    let mut driver = window.stage.drive();
    driver
        .chord(&Chord::parse("Ctrl+A")?, 0)
        .map_err(drive_error)?;
    driver.commit(&text).map_err(drive_error)?;
    window.rest()
}

#[then(regex = r#"^the inspector's "([^"]+)" holds "(.*)"$"#)]
fn row_holds(w: &mut UiWorld, name: String, want: String) -> StepResult {
    let want = unescape(&want);
    let field = w.window()?.node("Inspector", Some("textinput"), &name)?;
    (field.value == want)
        .then_some(())
        .ok_or_else(|| format!("the row holds {:?}", field.value))
}

#[then(regex = r#"^the inspector's new property row holds "(.*)"$"#)]
fn new_row_holds(w: &mut UiWorld, want: String) -> StepResult {
    let field = w
        .window()?
        .node("Inspector", Some("textinput"), "New property")?;
    (field.value == want)
        .then_some(())
        .ok_or_else(|| format!("the row holds {:?}", field.value))
}

#[then(regex = r#"^the documents panel shows (a|no) scroll bar$"#)]
fn documents_scroll(w: &mut UiWorld, want: String) -> StepResult {
    let shown = w
        .window()?
        .node("Documents", None, "Scroll the documents")
        .is_ok();
    (shown == (want == "a"))
        .then_some(())
        .ok_or_else(|| format!("a scroll bar shown: {shown}"))
}

#[then(regex = r#"^the inspector's comment box holds "(.*)"$"#)]
fn comment_box_holds(w: &mut UiWorld, want: String) -> StepResult {
    let field = w
        .window()?
        .node("Inspector", Some("textinput"), "New comment")?;
    (field.value == unescape(&want))
        .then_some(())
        .ok_or_else(|| format!("the comment box holds {:?}", field.value))
}

#[then("the window shows no refusal")]
fn no_refusal(w: &mut UiWorld) -> StepResult {
    let error = w.window()?.read("error").to_js_string();
    error
        .is_empty()
        .then_some(())
        .ok_or_else(|| format!("the window shows the refusal {error:?}"))
}

#[then(regex = r#"^the inspector lists the roles "([^"]*)"$"#)]
fn lists_roles(w: &mut UiWorld, names: String) -> StepResult {
    let shown = shown_with(w.window()?, "Inspector", "Role ")?;
    (shown == listed(&names))
        .then_some(())
        .ok_or_else(|| format!("the inspector lists the roles {shown:?}"))
}

#[then(regex = r#"^the inspector lists the signals "([^"]*)"$"#)]
fn lists_signals(w: &mut UiWorld, names: String) -> StepResult {
    let shown = shown_with(w.window()?, "Inspector", "Signal ")?;
    (shown == listed(&names))
        .then_some(())
        .ok_or_else(|| format!("the inspector lists the signals {shown:?}"))
}

/// Opens the flow panel's box of what a wire handles, as a person clicks
/// it.
#[given("the person has opened the list of what to wire in the flow panel")]
fn opens_wire_box(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let box_ = find(&tree, &|n| {
        n.name == "Wire" && n.role != "label" && n.key.is_some()
    })
    .cloned()
    .ok_or_else(|| format!("no box \"Wire\"; the window holds {}", names(&tree)))?;
    window.click_node(&box_)
}

/// Picks `signal` from the flow panel's box of what a wire handles: opens
/// it unless it is open, then clicks the signal's row in its list,
/// scrolling the list to it as a person does.
#[when(regex = r#"^the person picks the signal "([^"]+)" to wire in the flow panel$"#)]
fn picks_signal(w: &mut UiWorld, signal: String) -> StepResult {
    w.window()?.open_panel("Flow")?;
    if wire_row(w.window()?, &signal).is_none() {
        opens_wire_box(w)?;
    }
    let window = w.window()?;
    let row = wire_row(window, &signal).ok_or_else(|| {
        format!(
            "the open box of what to wire has no {signal:?}; the window holds {}",
            names(&window.tree())
        )
    })?;
    window.click_row(WIRE_LIST, &row)
}

/// The accessible name of the list the open box of what a wire handles
/// shows.
const WIRE_LIST: &str = "Wire list";

/// `signal`'s row in the open box of what a wire handles; none while it is
/// closed.
fn wire_row(window: &Window, signal: &str) -> Option<AccessibleNode> {
    let tree = window.tree();
    let list = find(&tree, &|n| n.name == WIRE_LIST && n.role == "list")?;
    find(list, &|n| n.name == signal && n.key.is_some()).cloned()
}

#[when(regex = r#"^the person binds the role "([^"]+)" to "([^"]+)" in the inspector$"#)]
fn binds_role(w: &mut UiWorld, role: String, name: String) -> StepResult {
    let window = w.window()?;
    let field = window.node(
        "Inspector",
        Some("textinput"),
        &format!("Bind role {role} to"),
    )?;
    window.enter(&field, &name)
}

#[then(regex = r#"^the frame "([^"]+)" shows the state "([^"]+)"$"#)]
fn frame_shows_state(w: &mut UiWorld, target: String, state: String) -> StepResult {
    frame_state_is(w.window()?, &target, &format!("State {state}"))
}

#[then(regex = r#"^the frame "([^"]+)" shows no state$"#)]
fn frame_shows_no_state(w: &mut UiWorld, target: String) -> StepResult {
    frame_state_is(w.window()?, &target, "No state")
}

fn frame_state_is(window: &Window, target: &str, want: &str) -> StepResult {
    let frame = window.frame(target)?;
    let label = ljos_drive::find_below(window.engine(), frame, "frameState")
        .ok_or("the frame does not say which state it shows")?;
    let text = window
        .engine()
        .read_by_name(label, "text")
        .to_js_string()
        .to_string();
    (text == want).then_some(()).ok_or_else(|| {
        format!(
            "the frame says {text:?}; the preview is {}",
            window.read_json("preview")
        )
    })
}

#[then(regex = r#"^the palette lists the slots "([^"]*)" of "([^"]+)"$"#)]
fn palette_lists_slots(w: &mut UiWorld, slots: String, block: String) -> StepResult {
    let window = w.window()?;
    for slot in list(&slots) {
        window.node("Palette", None, &format!("Slot {slot} of {block}"))?;
    }
    Ok(())
}

#[when(regex = r#"^the person contributes "(.*)" to the slot "([^"]+)" from the palette$"#)]
fn contributes(w: &mut UiWorld, text: String, slot: String) -> StepResult {
    let text = unescape(&text);
    let window = w.window()?;
    let field = window.node(
        "Palette",
        Some("textinput"),
        &format!("Contribution to {slot}"),
    )?;
    window.enter(&field, &text)
}

#[given(regex = r#"^the person has pressed "([^"]+)"$"#)]
#[when(regex = r#"^the person presses "([^"]+)"$"#)]
fn presses(w: &mut UiWorld, chord: String) -> StepResult {
    let window = w.window()?;
    let chord = Chord::parse(&chord)?;
    window.stage.drive().chord(&chord, 0).map_err(drive_error)?;
    window.rest()
}

#[when("the person switches to another window")]
fn leaves_window(w: &mut UiWorld) -> StepResult {
    window_active(w, false)
}

#[when("the person comes back to the window")]
fn comes_back(w: &mut UiWorld) -> StepResult {
    window_active(w, true)
}

fn window_active(w: &mut UiWorld, active: bool) -> StepResult {
    let window = w.window()?;
    let stage = &mut window.stage;
    stage.scene.set_window_active(&stage.engine, active);
    window.rest()
}

#[when(regex = r#"^the person presses "([^"]+)" without waiting between them$"#)]
fn presses_quickly(w: &mut UiWorld, chords: String) -> StepResult {
    let window = w.window()?;
    for chord in chords.split(", ") {
        let chord = Chord::parse(chord)?;
        window.stage.drive().chord(&chord, 0).map_err(drive_error)?;
    }
    window.rest()
}

#[when(regex = r#"^the person replaces "(.*)" with "(.*)" in the code view$"#)]
fn replaces_in_code(w: &mut UiWorld, old: String, new: String) -> StepResult {
    let (old, new) = (unescape(&old), unescape(&new));
    let window = w.window()?;
    let code = window.code()?;
    window.click_shown(&code)?;
    let item = window
        .stage
        .scene
        .accessible_item(code.key.ok_or("the code view has no key")?)
        .ok_or("the code view is not an item")?;
    let text = code.value.clone();
    let at = text
        .find(&old)
        .ok_or_else(|| format!("the code view does not have {old:?}"))?;
    // The place as the editor counts it, in UTF-16 units, as a person's
    // selection by mouse sets it.
    let start = text[..at].encode_utf16().count();
    let end = start + old.encode_utf16().count();
    let engine = window.engine().clone();
    engine
        .invoke_by_name(
            item,
            "select",
            &[Value::Int(start as i32), Value::Int(end as i32)],
        )
        .map_err(|e| e.to_string())?;
    if new.is_empty() {
        // Typing nothing is no edit; a person deletes the selection.
        window
            .stage
            .drive()
            .chord(&Chord::parse("Backspace")?, 0)
            .map_err(drive_error)?;
    } else {
        window.stage.drive().commit(&new).map_err(drive_error)?;
    }
    window.rest()
}

#[when(regex = r#"^the person enters "([^"]+)" as the new document's name$"#)]
fn enters_new_name(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Documents", Some("textinput"), "New document's name")?;
    window.click_shown(&field)?;
    window.stage.drive().commit(&name).map_err(drive_error)?;
    window.rest()
}

// Focus without a click: a click would drop a text selection by itself, and
// the object the layers panel chose would seem safe from the next key.
#[when(regex = r#"^the person tabs to the code view and types "(.*)"$"#)]
fn tabs_and_types(w: &mut UiWorld, new: String) -> StepResult {
    let new = unescape(&new);
    let window = w.window()?;
    let code = window.code()?;
    let item = window
        .stage
        .scene
        .accessible_item(code.key.ok_or("the code view has no key")?)
        .ok_or("the code view is not an item")?;
    let engine = window.engine().clone();
    engine
        .invoke_by_name(item, "forceActiveFocus", &[])
        .map_err(|e| e.to_string())?;
    window.stage.drive().commit(&new).map_err(drive_error)?;
    window.rest()
}

#[given(regex = r#"^an agent has called the tool "([^"]+)" with:$"#)]
#[when(regex = r#"^an agent calls the tool "([^"]+)" with:$"#)]
fn agent_calls(w: &mut UiWorld, tool: String, step: &Step) -> StepResult {
    call_as(w, None, &tool, step)
}

#[given(regex = r#"^an agent named "([^"]+)" has called the tool "([^"]+)" with:$"#)]
#[when(regex = r#"^an agent named "([^"]+)" calls the tool "([^"]+)" with:$"#)]
fn named_agent_calls(w: &mut UiWorld, name: String, tool: String, step: &Step) -> StepResult {
    call_as(w, Some(name), &tool, step)
}

/// An agent's call beside the window's: `client` is the name it gave
/// itself, none for one that gave none.
fn call_as(w: &mut UiWorld, client: Option<String>, tool: &str, step: &Step) -> StepResult {
    let args: Json = serde_json::from_str(&docstring(step)?).map_err(|e| e.to_string())?;
    let window = w.window()?;
    let ctx = CallCtx {
        client,
        ..CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate]))
    };
    window
        .server
        .call(tool, &ctx, args)
        .map_err(|e| format!("the agent was refused: {}", e.to_value()))?;
    window.rest()
}

#[when(regex = r#"^the person chooses the target "([^"]+)"$"#)]
fn chooses_target(w: &mut UiWorld, target: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let button = find(&tree, &|n| {
        n.name == target && n.key.is_some() && n.role.contains("button")
    })
    .cloned()
    .ok_or_else(|| format!("no target {target:?}; the window holds {}", names(&tree)))?;
    window.click_node(&button)
}

#[given(regex = r#"^the person has clicked the frame "([^"]+)" at (\d+), (\d+)$"#)]
#[when(regex = r#"^the person clicks the frame "([^"]+)" at (\d+), (\d+)$"#)]
fn clicks_frame(w: &mut UiWorld, target: String, x: f64, y: f64) -> StepResult {
    let window = w.window()?;
    let bounds = window
        .bounds(&|role, name| role == "image" && name == target)
        .ok_or_else(|| format!("the canvas has no frame {target:?} drawn"))?;
    let zoom = bounds.width() / window.read_json("preview")["width"].as_f64().unwrap_or(1.0);
    let at = Point::new(bounds.x0 + x * zoom, bounds.y0 + y * zoom);
    // The canvas scrolls: the point must be in the window to be clicked.
    if !(0.0..WIDTH).contains(&at.x) || !(0.0..HEIGHT).contains(&at.y) {
        return Err(format!(
            "{x}, {y} of the frame is at {at:?}, outside the window"
        ));
    }
    window
        .stage
        .drive()
        .click(at, MouseButtons::LEFT, 1)
        .map_err(drive_error)?;
    window.rest()
}

/// A click with Shift held, which adds to the selection.
fn shift_click(window: &mut Window, at: Point) -> StepResult {
    let shift = ljos_drive::parse_modifiers("Shift")?;
    let mut driver = window.stage.drive();
    let t = driver.now();
    let mut moved = PointerEvent::moved(at.x, at.y, MouseButtons::empty(), t);
    moved.modifiers = shift;
    let mut pressed = PointerEvent::press(at.x, at.y, t).with_button(MouseButtons::LEFT);
    pressed.modifiers = shift;
    let mut released = PointerEvent::release(at.x, at.y, t).with_button(MouseButtons::LEFT);
    released.modifiers = shift;
    driver.send_pointer(&moved);
    driver.send_pointer(&pressed);
    // A hand is still down when the selection's answer comes.
    window.rest()?;
    let mut driver = window.stage.drive();
    driver.send_pointer(&released);
    driver.settle().map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person clicks the frame "([^"]+)" at (\d+), (\d+) holding Shift$"#)]
fn shift_clicks_frame(w: &mut UiWorld, target: String, x: f64, y: f64) -> StepResult {
    let window = w.window()?;
    let at = frame_point(window, &target, x, y)?;
    shift_click(window, at)
}

#[when(regex = r#"^the person selects "([^"]+)" in the layers panel holding Shift$"#)]
fn shift_selects_layer(w: &mut UiWorld, label: String) -> StepResult {
    let window = w.window()?;
    let row = window
        .shown_layers()?
        .into_iter()
        .find(|n| n.name == label)
        .ok_or_else(|| format!("the layers panel lists {:?}", window.layers()))?;
    let key = row.key.ok_or("the row has no key")?;
    let at = window
        .stage
        .drive()
        .aim_at_node(key)
        .map_err(|why| format!("{why:?}"))?;
    shift_click(window, at)
}

/// Where a point of the frame is in the window.
fn frame_point(window: &Window, target: &str, x: f64, y: f64) -> Result<Point, String> {
    let bounds = window
        .bounds(&|role, name| role == "image" && name == target)
        .ok_or_else(|| format!("the canvas has no frame {target:?} drawn"))?;
    let zoom = bounds.width() / window.read_json("preview")["width"].as_f64().unwrap_or(1.0);
    Ok(Point::new(bounds.x0 + x * zoom, bounds.y0 + y * zoom))
}

#[given(
    regex = r#"^the person has dragged in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+)$"#
)]
#[when(regex = r#"^the person drags in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+)$"#)]
fn drags_in_frame(
    w: &mut UiWorld,
    target: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> StepResult {
    let window = w.window()?;
    let from = frame_point(window, &target, x0, y0)?;
    let to = frame_point(window, &target, x1, y1)?;
    // A person's press reaches the geometry before the hand moves on; the
    // pause is what a frame of a real drag gives it.
    window
        .stage
        .drive()
        .press(from, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()?;
    window.stage.drive().move_to(to).map_err(drive_error)?;
    window
        .stage
        .drive()
        .release(to, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()
}

/// A hand faster than the press's read of the running app.
#[when(
    regex = r#"^the person drags in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+) at once$"#
)]
fn drags_in_frame_at_once(
    w: &mut UiWorld,
    target: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> StepResult {
    let window = w.window()?;
    let from = frame_point(window, &target, x0, y0)?;
    let to = frame_point(window, &target, x1, y1)?;
    let mut drive = window.stage.drive();
    drive.press(from, MouseButtons::LEFT).map_err(drive_error)?;
    drive.move_to(to).map_err(drive_error)?;
    drive.release(to, MouseButtons::LEFT).map_err(drive_error)?;
    window.rest()
}

#[given("nothing has happened")]
#[when("nothing has happened")]
fn nothing_happened(_: &mut UiWorld) {}

/// What an agent asks in a table's row, where a doc string cannot go.
fn agent_call(w: &mut UiWorld, client: Option<&str>, tool: &str, args: Json) -> StepResult {
    let window = w.window()?;
    let ctx = CallCtx {
        client: client.map(str::to_owned),
        ..CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate]))
    };
    window
        .server
        .call(tool, &ctx, args)
        .map_err(|e| format!("the agent was refused: {}", e.to_value()))?;
    window.rest()
}

#[given(regex = r#"^Ada holds "([^"]+)"$"#)]
fn ada_holds(w: &mut UiWorld, id: String) -> StepResult {
    let args = serde_json::json!({"doc": "Main.qml", "nodes": [{"id": id}], "hold": true});
    agent_call(w, Some("Ada"), "node_select", args)
}

#[when(regex = r#"^an agent selects "([^"]+)"$"#)]
fn agent_selects(w: &mut UiWorld, id: String) -> StepResult {
    let args = serde_json::json!({"doc": "Main.qml", "nodes": [{"id": id}]});
    agent_call(w, None, "node_select", args)
}

/// The hand comes up where the hold left it.
fn lets_go(w: &mut UiWorld) -> StepResult {
    let to = w.held.ok_or("no hand is holding the canvas")?;
    let window = w.window()?;
    window
        .stage
        .drive()
        .release(to, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^an agent selects "([^"]+)" and the hand lets go$"#)]
fn agent_selects_then_go(w: &mut UiWorld, id: String) -> StepResult {
    agent_selects(w, id)?;
    lets_go(w)
}

/// A read that answers before the agent's selection is heard cannot show
/// the order this is about, so the overlay is put where a slow read leaves it.
#[when(regex = r#"^the drop waits on a read and an agent selects "([^"]+)"$"#)]
fn drop_waits_then_select(w: &mut UiWorld, id: String) -> StepResult {
    let window = w.window()?;
    let overlay = overlay_part(window, "overlay")?;
    let engine = window.engine();
    for (name, value) in [
        ("mode", Value::from("")),
        ("dropping", Value::Bool(true)),
        ("dropWaits", Value::Bool(true)),
        ("reads", Value::Int(1)),
    ] {
        engine
            .write_by_name(overlay, name, value)
            .map_err(|e| e.to_string())?;
    }
    agent_selects(w, id)
}

#[then("the canvas has gone with the preview")]
fn canvas_gone(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    match overlay_part(window, "overlay") {
        Err(_) => Ok(()),
        Ok(_) => Err("the canvas is still there".to_owned()),
    }
}

#[when("the preview closes")]
fn preview_closes(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let studio = window.server.studio();
    let close = studio
        .lock()
        .map_err(|_| "the studio's lock is poisoned")?
        .preview_closer();
    close();
    window.rest()
}

#[when("the preview closes and the hand lets go")]
fn preview_closes_then_go(w: &mut UiWorld) -> StepResult {
    preview_closes(w)?;
    lets_go(w)
}

#[when("the window takes the press away")]
fn press_taken(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let overlay = overlay_part(window, "overlay")?;
    window
        .engine()
        .emit_by_name(overlay, "canceled", &[])
        .map_err(|e| e.to_string())?;
    window.rest()
}

#[when(
    regex = r#"^the person lets a drag go in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+), not waiting$"#
)]
fn drag_not_waiting(
    w: &mut UiWorld,
    target: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> StepResult {
    let window = w.window()?;
    let from = frame_point(window, &target, x0, y0)?;
    let to = frame_point(window, &target, x1, y1)?;
    let mut drive = window.stage.drive();
    drive.press(from, MouseButtons::LEFT).map_err(drive_error)?;
    drive.move_to(to).map_err(drive_error)?;
    drive.release(to, MouseButtons::LEFT).map_err(drive_error)?;
    Ok(())
}

#[when(
    regex = r#"^the person presses "([^"]+)" and drags in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+)$"#
)]
fn nudge_then_drag(
    w: &mut UiWorld,
    chord: String,
    target: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> StepResult {
    presses(w, chord)?;
    drags_in_frame(w, target, x0, y0, x1, y1)
}

/// Past the wait after a key press before the overlay reads again.
#[when("the canvas has had time to settle")]
fn canvas_settled(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    window.advance(400)?;
    window.rest()
}

#[then("no ghost is left on the canvas")]
fn no_ghost(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let overlay = overlay_part(window, "overlay")?;
    let engine = window.engine();
    let dropping = engine.read_by_name(overlay, "dropping");
    let ghost = engine.read_by_name(overlay, "ghost");
    if ghost != Value::Null || dropping != Value::Bool(false) {
        return Err(format!(
            "ghost {ghost:?} still shown, dropping {dropping:?}"
        ));
    }
    let frame = window.frame("Desktop")?;
    if let Some(outline) = ljos_drive::find_below(engine, frame, "selectionOutline") {
        let opacity = engine.read_by_name(outline, "opacity").to_number();
        if opacity != 1.0 {
            return Err(format!("the outline is still dim, at {opacity}"));
        }
    }
    Ok(())
}

/// Each frame's outline while nothing covers it: where a person saw it.
struct OutlineWatch {
    overlay: ObjectId,
    frame: ObjectId,
    bare: Vec<(f64, f64)>,
}

impl ljos_drive::Host for OutlineWatch {
    fn after_frame(&mut self, engine: &Engine, _scene: &mut ljos_quick::QuickScene) {
        // Before the hand lets go the ghost is not yet the drop's to keep.
        if !matches!(engine.read_by_name(self.overlay, "mode"), Value::String(m) if m.is_empty())
            || !matches!(engine.read_by_name(self.overlay, "ghost"), Value::Null)
        {
            return;
        }
        let Some(outline) = ljos_drive::find_below(engine, self.frame, "selectionOutline") else {
            return;
        };
        let zoom = engine.read_by_name(self.overlay, "zoom").to_number();
        self.bare.push((
            engine.read_by_name(outline, "x").to_number() / zoom,
            engine.read_by_name(outline, "y").to_number() / zoom,
        ));
    }
}

#[when(
    regex = r#"^the person flicks in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+), watching the outline$"#
)]
fn flicks_in_frame(
    w: &mut UiWorld,
    target: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> StepResult {
    let window = w.window()?;
    let from = frame_point(window, &target, x0, y0)?;
    let to = frame_point(window, &target, x1, y1)?;
    let overlay = overlay_part(window, "overlay")?;
    let mut watch = OutlineWatch {
        overlay,
        frame: window.frame("Desktop")?,
        bare: Vec::new(),
    };
    let mut drive = window.stage.drive().hosted(&mut watch);
    drive.press(from, MouseButtons::LEFT).map_err(drive_error)?;
    drive.move_to(to).map_err(drive_error)?;
    drive.release(to, MouseButtons::LEFT).map_err(drive_error)?;
    // Not `finish_promised`, which takes every answer before the next frame:
    // a person sees a frame between the read's answer and the write's.
    let deadline = std::time::Instant::now() + PATIENCE;
    while drive.engine.read_by_name(overlay, "ghost") != Value::Null
        && std::time::Instant::now() < deadline
    {
        drive.settle().map_err(drive_error)?;
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    drive.finish_promised().map_err(drive_error)?;
    w.outlines = watch.bare;
    Ok(())
}

#[then(regex = r"^the outline was never shown back at (\d+), (\d+) once the hand let go$")]
fn outline_never_back(w: &mut UiWorld, x: f64, y: f64) -> StepResult {
    let close = |a: f64, b: f64| (a - b).abs() < 0.5;
    // Without a frame at all, this would say nothing about the flick.
    if w.outlines.is_empty() {
        return Err("no frame showed the outline once the drop was done".to_owned());
    }
    match w.outlines.iter().find(|o| close(o.0, x) && close(o.1, y)) {
        Some(_) => Err(format!("the outline went back: {:?}", w.outlines)),
        None => Ok(()),
    }
}

#[when(
    regex = r#"^the person presses in the frame "([^"]+)" at (\d+), (\d+) and holds it at (\d+), (\d+)$"#
)]
fn holds_in_frame(
    w: &mut UiWorld,
    target: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> StepResult {
    let window = w.window()?;
    let from = frame_point(window, &target, x0, y0)?;
    let to = frame_point(window, &target, x1, y1)?;
    window
        .stage
        .drive()
        .press(from, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()?;
    window.stage.drive().move_to(to).map_err(drive_error)?;
    window.rest()?;
    w.held = Some(to);
    Ok(())
}

fn overlay_part(window: &Window, name: &str) -> Result<ObjectId, String> {
    let frame = window.frame("Desktop")?;
    ljos_drive::find_below(window.engine(), frame, name)
        .ok_or_else(|| format!("the canvas has no {name}"))
}

#[then(regex = r#"^the file "([^"]+)" does not say "(.*)"$"#)]
fn file_does_not_say(w: &mut UiWorld, name: String, want: String) -> StepResult {
    let got = std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| e.to_string())?;
    if got.contains(&want) {
        return Err(format!("`{name}` says {want:?}:\n{got}"));
    }
    Ok(())
}

#[then(regex = r#"^the window says "(.*)"$"#)]
fn window_says(w: &mut UiWorld, want: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == want)
        .map(drop)
        .ok_or_else(|| format!("the window does not say {want:?}"))
}

#[then(regex = r#"^the window does not say "(.*)"$"#)]
fn window_does_not_say(w: &mut UiWorld, want: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == want)
        .is_none()
        .then_some(())
        .ok_or_else(|| format!("the window still says {want:?}"))
}

#[then(regex = r#"^the file "([^"]+)" says "(.*)"$"#)]
fn file_says(w: &mut UiWorld, name: String, want: String) -> StepResult {
    let want = unescape(&want);
    let got = std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| e.to_string())?;
    got.contains(&want)
        .then_some(())
        .ok_or_else(|| format!("`{name}` does not say {want:?}:\n{got}"))
}

#[then(regex = r#"^the canvas outlines the selection at (\d+), (\d+) sized (\d+) by (\d+)$"#)]
fn outline_at(w: &mut UiWorld, x: f64, y: f64, width: f64, height: f64) -> StepResult {
    let window = w.window()?;
    let outline = overlay_part(window, "selectionOutline")?;
    let zoom = window
        .engine()
        .read_by_name(overlay_part(window, "overlay")?, "zoom")
        .to_number();
    let read = |name: &str| window.engine().read_by_name(outline, name).to_number() / zoom;
    let got = (read("x"), read("y"), read("width"), read("height"));
    let close = |a: f64, b: f64| (a - b).abs() < 0.5;
    (close(got.0, x) && close(got.1, y) && close(got.2, width) && close(got.3, height))
        .then_some(())
        .ok_or_else(|| format!("the outline is at {got:?}"))
}

#[then(regex = r#"^the canvas shows (\d+) resize handles$"#)]
fn handles_shown(w: &mut UiWorld, want: usize) -> StepResult {
    let window = w.window()?;
    let overlay = overlay_part(window, "overlay")?;
    let got = window
        .engine()
        .read_by_name(overlay, "handlesShown")
        .to_number() as usize;
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("{got} handles"))
}

#[then(regex = r#"^the canvas shows (\d+) move arrows$"#)]
fn arrows_shown(w: &mut UiWorld, want: usize) -> StepResult {
    let window = w.window()?;
    let overlay = overlay_part(window, "overlay")?;
    let got = window
        .engine()
        .read_by_name(overlay, "arrowsShown")
        .to_number() as usize;
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("{got} arrows"))
}

#[then(regex = r#"^the canvas shows a ([xy]) guide at (\d+)$"#)]
fn guide_at(w: &mut UiWorld, axis: String, at: f64) -> StepResult {
    let window = w.window()?;
    let guide = overlay_part(window, &format!("guide{}", axis.to_uppercase()))?;
    let zoom = window
        .engine()
        .read_by_name(overlay_part(window, "overlay")?, "zoom")
        .to_number();
    let got = window.engine().read_by_name(guide, &axis).to_number() / zoom;
    let visible = window.engine().read_by_name(guide, "visible").to_boolean();
    (visible && (got - at).abs() < 0.5)
        .then_some(())
        .ok_or_else(|| format!("the guide is at {got}, visible {visible}"))
}

#[then(regex = r#"^the canvas says "(.+)"$"#)]
fn canvas_says(w: &mut UiWorld, want: String) -> StepResult {
    let window = w.window()?;
    let hint =
        ljos_drive::find_below(window.engine(), window.root, "canvasHint").ok_or("no hint")?;
    let got = window
        .engine()
        .read_by_name(hint, "text")
        .to_js_string()
        .to_string();
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("the canvas says {got:?}"))
}

#[then("the canvas's words are clear of its recording bar")]
fn hint_clear_of_bar(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let engine = window.engine();
    let part =
        |name: &str| ljos_drive::find_below(engine, window.root, name).ok_or(format!("no {name}"));
    let (hint, bar) = (part("canvasHint")?, part("recordingBar")?);
    if !engine.read_by_name(bar, "visible").to_boolean() {
        return Err("the recording bar is not showing".to_owned());
    }
    let hint_bottom = engine.read_by_name(hint, "y").to_number()
        + engine.read_by_name(hint, "height").to_number();
    let bar_top = engine.read_by_name(bar, "y").to_number();
    (hint_bottom <= bar_top)
        .then_some(())
        .ok_or_else(|| format!("the words end at {hint_bottom}, the bar starts at {bar_top}"))
}

#[when(regex = r#"^nobody touches the window for (\d+) seconds?$"#)]
fn nobody_touches(w: &mut UiWorld, seconds: u64) -> StepResult {
    let window = w.window()?;
    window.rest()?;
    let before = window.stage.state.clone();
    window.advance(seconds * 1000)?;
    let after = &window.stage.state;
    window.idle = Some(Idle {
        wake_before: before.wake,
        wake_after: after.wake,
        shown: after.shown - before.shown,
    });
    Ok(())
}

/// Narrows the tokens panel to `token`, as a person does to find a role
/// among the module's hundred and more.
fn filter_tokens(window: &mut Window, token: &str) -> StepResult {
    let field = window.node("Tokens", Some("textinput"), "Filter tokens")?;
    if field.value != token {
        window.enter(&field, token)?;
    }
    Ok(())
}

/// The role a field of the tokens panel sets: `Colors.accent light` is
/// `Colors.accent`.
fn token_of(field: &str) -> &str {
    field
        .strip_suffix(" light")
        .or_else(|| field.strip_suffix(" dark"))
        .unwrap_or(field)
}

#[given(regex = r#"^the person has set "([^"]+)" to "(.*)" in the tokens panel$"#)]
#[when(regex = r#"^the person sets "([^"]+)" to "(.*)" in the tokens panel$"#)]
fn sets_token(w: &mut UiWorld, name: String, value: String) -> StepResult {
    let window = w.window()?;
    filter_tokens(window, token_of(&name))?;
    let field = window.node("Tokens", Some("textinput"), &name)?;
    window.enter(&field, &value)
}

// ------------------------------------------------------------------- then

#[then(regex = r#"^the tokens panel shows "([^"]+): ([^"]*)"$"#)]
fn tokens_show(w: &mut UiWorld, name: String, value: String) -> StepResult {
    let window = w.window()?;
    filter_tokens(window, token_of(&name))?;
    let field = window.node("Tokens", Some("textinput"), &name)?;
    (field.value == value).then_some(()).ok_or_else(|| {
        format!(
            "the tokens panel shows {name} as {:?}; the last refusal {:?}",
            field.value,
            window.read("error").to_js_string()
        )
    })
}

#[then(regex = r#"^the tokens panel says "(.+)"$"#)]
fn tokens_say(w: &mut UiWorld, pattern: String) -> StepResult {
    let pattern = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
    let window = w.window()?;
    window.open_panel("Tokens")?;
    let tree = window.tree();
    let panel = find(&tree, &|n| n.name == "Tokens" && n.role != "label")
        .ok_or("no tokens panel in the window")?;
    find(panel, &|n| pattern.is_match(&n.name))
        .map(drop)
        .ok_or_else(|| {
            format!(
                "the tokens panel says none of {}; the last refusal {:?}, history {}",
                names(panel),
                window.read("error").to_js_string(),
                window.read_json("done")
            )
        })
}

#[when(regex = r#"^the person drags the layer "([^"]+)" onto "([^"]+)"$"#)]
fn drags_layer(w: &mut UiWorld, from: String, onto: String) -> StepResult {
    let window = w.window()?;
    let row = |window: &mut Window, name: &str| -> Result<Point, String> {
        let node = window
            .shown_layers()?
            .into_iter()
            .find(|n| n.name == name)
            .ok_or_else(|| format!("no layer {name:?}"))?;
        let key = node.key.ok_or("the row has no key")?;
        window
            .stage
            .drive()
            .aim_at_node(key)
            .map_err(|why| format!("{why:?}"))
    };
    let start = row(window, &from)?;
    let end = row(window, &onto)?;
    let mut driver = window.stage.drive();
    driver
        .press(start, MouseButtons::LEFT)
        .map_err(drive_error)?;
    // Past the threshold wherever the row lies, so a drag back to its own row
    // is still a drag.
    let aside = Point::new(start.x + 12.0, start.y + 12.0);
    driver.move_to(aside).map_err(drive_error)?;
    driver.move_to(end).map_err(drive_error)?;
    driver
        .release(end, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person double-clicks the layer "([^"]+)"$"#)]
fn double_clicks_layer(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let node = window
        .shown_layers()?
        .into_iter()
        .find(|n| n.name == name)
        .ok_or_else(|| format!("no layer {name:?}"))?;
    let key = node.key.ok_or("the row has no key")?;
    let mut driver = window.stage.drive();
    let at = driver.aim_at_node(key).map_err(|why| format!("{why:?}"))?;
    driver
        .click(at, MouseButtons::LEFT, 2)
        .map_err(drive_error)?;
    window.rest()
}

#[given(regex = r#"^the person has double-clicked the frame "([^"]+)" at (\d+), (\d+)$"#)]
#[when(regex = r#"^the person double-clicks the frame "([^"]+)" at (\d+), (\d+)$"#)]
fn double_clicks_frame(w: &mut UiWorld, target: String, x: f64, y: f64) -> StepResult {
    let window = w.window()?;
    let at = frame_point(window, &target, x, y)?;
    window
        .stage
        .drive()
        .click(at, MouseButtons::LEFT, 1)
        .map_err(drive_error)?;
    // The first click's selection and its box come back before the second,
    // as they do between a person's two clicks.
    window.rest()?;
    window
        .stage
        .drive()
        .click(at, MouseButtons::LEFT, 1)
        .map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person writes "(.*)" on the canvas and presses "([^"]+)"$"#)]
fn writes_on_canvas(w: &mut UiWorld, text: String, key: String) -> StepResult {
    let text = unescape(&text);
    let window = w.window()?;
    let chord = Chord::parse(&key)?;
    // What was there is selected when the field opens: typing replaces it.
    window.stage.drive().commit(&text).map_err(drive_error)?;
    window.rest()?;
    window.stage.drive().chord(&chord, 0).map_err(drive_error)?;
    window.rest()
}

#[given(regex = r#"^the person has typed "(.*)" on the canvas$"#)]
fn has_typed_on_canvas(w: &mut UiWorld, text: String) -> StepResult {
    let text = unescape(&text);
    let window = w.window()?;
    window.stage.drive().commit(&text).map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person types "([^"]+)" on the canvas$"#)]
fn types_on_canvas(w: &mut UiWorld, key: String) -> StepResult {
    let window = w.window()?;
    let chord = Chord::parse(&key)?;
    window.stage.drive().chord(&chord, 0).map_err(drive_error)?;
    window.rest()
}

#[when(
    regex = r#"^the person holds "([^"]+)" and drags in the frame "([^"]+)" from (\d+), (\d+) to (\d+), (\d+)$"#
)]
fn holds_and_drags(
    w: &mut UiWorld,
    key: String,
    target: String,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
) -> StepResult {
    let window = w.window()?;
    let before = frame_bounds(window, &target)?;
    let from = frame_point(window, &target, from_x, from_y)?;
    let to = frame_point(window, &target, to_x, to_y)?;
    let chord = Chord::parse(&key)?;
    window.stage.drive().key_down(&chord).map_err(drive_error)?;
    window.rest()?;
    window
        .stage
        .drive()
        .press(from, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()?;
    // In the steps a hand makes: a view starts following past a threshold.
    let aside = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    window.stage.drive().move_to(aside).map_err(drive_error)?;
    window.stage.drive().move_to(to).map_err(drive_error)?;
    window
        .stage
        .drive()
        .release(to, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()?;
    window.stage.drive().key_up(&chord).map_err(drive_error)?;
    window.rest()?;
    w.frame_before = Some(before);
    Ok(())
}

#[then(regex = r#"^the canvas's text holds "(.*)"$"#)]
fn canvas_text_holds(w: &mut UiWorld, want: String) -> StepResult {
    let want = unescape(&want);
    let tree = w.window()?.tree();
    let field = find(&tree, &|n| n.name == "Text of the selected object")
        .ok_or("no text field is open on the canvas")?;
    (field.value == want)
        .then_some(())
        .ok_or_else(|| format!("the canvas's text holds {:?}", field.value))
}

#[when(regex = r#"^the person holds "([^"]+)"$"#)]
fn holds_key(w: &mut UiWorld, key: String) -> StepResult {
    let window = w.window()?;
    window
        .stage
        .drive()
        .key_down(&Chord::parse(&key)?)
        .map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person lets go of "([^"]+)"$"#)]
fn lets_go_of_key(w: &mut UiWorld, key: String) -> StepResult {
    let window = w.window()?;
    window
        .stage
        .drive()
        .key_up(&Chord::parse(&key)?)
        .map_err(drive_error)?;
    window.rest()
}

// A write that fails is what a person meets on a full disk or a read-only
// checkout, and no gesture can bring it about.
#[given("the project's folder takes no writes")]
fn folder_takes_no_writes(w: &mut UiWorld) -> StepResult {
    set_folder_read_only(w, true)
}

#[when("the project's folder takes writes again")]
fn folder_takes_writes(w: &mut UiWorld) -> StepResult {
    set_folder_read_only(w, false)
}

fn set_folder_read_only(w: &UiWorld, read_only: bool) -> StepResult {
    let mut permissions = std::fs::metadata(w.dir.path())
        .map_err(|e| e.to_string())?
        .permissions();
    permissions.set_readonly(read_only);
    std::fs::set_permissions(w.dir.path(), permissions).map_err(|e| e.to_string())
}

#[then("the canvas has no text being written")]
fn no_canvas_text(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let field = overlay_part(window, "canvasText")?;
    if window.engine().read_by_name(field, "visible").to_boolean() {
        return Err("a text field is open on the canvas".into());
    }
    Ok(())
}

#[when(regex = r#"^the person enters "([^"]+)" as the layer's name$"#)]
fn enters_layer_name(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let field = find(&tree, &|n| {
        n.name.starts_with("Name of ") && n.key.is_some()
    })
    .cloned()
    .ok_or_else(|| format!("no name field; the window holds {}", names(&tree)))?;
    window.enter(&field, &name)
}

#[then(regex = r#"^the layers panel has the button "([^"]+)"$"#)]
fn layers_has_button(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == name && n.role == "button")
        .map(|_| ())
        .ok_or_else(|| format!("no button {name:?}; the window holds {}", names(&tree)))
}

#[then(regex = r#"^the history does not list "([^"]+)"$"#)]
fn history_lists_not(w: &mut UiWorld, label: String) -> StepResult {
    let done = w.window()?.read_json("done");
    (!done.to_string().contains(&label))
        .then_some(())
        .ok_or_else(|| format!("the history lists {done}"))
}

#[then(regex = r#"^the file "([^"]+)" says:$"#)]
fn file_says_all(w: &mut UiWorld, name: String, step: &Step) -> StepResult {
    let got = std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| e.to_string())?;
    let want = file_text(step)?;
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("`{name}` says:\n{got}"))
}

#[then(regex = r#"^the inspector has the button "([^"]+)"$"#)]
fn inspector_has_button(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == name && n.role == "button")
        .map(|_| ())
        .ok_or_else(|| format!("no button {name:?}; the window holds {}", names(&tree)))
}

#[then(regex = r#"^the inspector has no row "([^"]+)"$"#)]
fn inspector_no_row(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == name && n.role == "textinput")
        .map_or(Ok(()), |_| Err(format!("the inspector has a row {name:?}")))
}

#[then(regex = r#"^the inspector has no button "([^"]+)"$"#)]
fn inspector_has_no_button(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == name && n.role == "button")
        .map_or(Ok(()), |_| Err(format!("a button {name:?}")))
}

#[then(
    regex = r#"^the inspector's "([^"]+)" has (a picker|no picker), painted "([^"]*)", with the note "([^"]*)"$"#
)]
fn colour_row_offers(
    w: &mut UiWorld,
    row: String,
    picker: String,
    paint: String,
    note: String,
) -> StepResult {
    let button = format!("Pick {row}");
    let tree = w.window()?.tree();
    let Some(swatch) = find(&tree, &|n| n.name == button && n.role == "button").cloned() else {
        return (picker == "no picker")
            .then_some(())
            .ok_or_else(|| format!("no button {button:?}; the window holds {}", names(&tree)));
    };
    if picker == "no picker" {
        return Err(format!("a button {button:?}"));
    }
    if swatch.description != paint {
        return Err(format!("the swatch paints {:?}", swatch.description));
    }
    clicks_button(w, button)?;
    let tree = w.window()?.tree();
    let says = find(&tree, &|n| n.name.starts_with("Worked out by")).map(|n| n.name.as_str());
    (says.unwrap_or("") == note)
        .then_some(())
        .ok_or_else(|| format!("the picker's note says {says:?}"))
}

#[when(regex = r#"^the person enters "([^"]*)" in the colour picker$"#)]
fn enters_colour(w: &mut UiWorld, value: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let field = find(&tree, &|n| n.name == "Colour" && n.key.is_some())
        .cloned()
        .ok_or_else(|| format!("no colour field; the window holds {}", names(&tree)))?;
    window.enter(&field, &value)
}

#[when(
    regex = r#"^the person drags the picker's "([^"]+)" to its middle while an agent selects "([^"]+)"$"#
)]
fn drags_picker_while_selecting(w: &mut UiWorld, name: String, other: String) -> StepResult {
    let doc = w.window()?.read_json("doc");
    let args = serde_json::json!({"doc": doc, "nodes": [{"id": other}]});
    drags_picker_while(w, &name, "node_select", args)
}

#[when(
    regex = r#"^the person drags the picker's "([^"]+)" to its middle while an agent calls "([^"]+)" with '(.+)'$"#
)]
fn drags_picker_while_calling(
    w: &mut UiWorld,
    name: String,
    tool: String,
    args: String,
) -> StepResult {
    let args = serde_json::from_str(&args).map_err(|e| format!("{e}"))?;
    drags_picker_while(w, &name, &tool, args)
}

fn drags_picker_while(w: &mut UiWorld, name: &str, tool: &str, args: Json) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let slider = find(&tree, &|n| n.name == name && n.role == "slider")
        .cloned()
        .ok_or_else(|| format!("no slider {name:?}; the window holds {}", names(&tree)))?;
    let key = slider.key.ok_or("the slider has no key")?;
    let mut driver = window.stage.drive();
    let middle = driver.aim_at_node(key).map_err(|why| format!("{why:?}"))?;
    driver
        .press(middle, MouseButtons::LEFT)
        .map_err(drive_error)?;
    driver
        .move_to(Point::new(middle.x - 12.0, middle.y))
        .map_err(drive_error)?;
    driver.move_to(middle).map_err(drive_error)?;
    let ctx = CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate]));
    window
        .server
        .call(tool, &ctx, args)
        .map_err(|e| format!("the agent was refused: {}", e.to_value()))?;
    window.rest()?;
    window
        .stage
        .drive()
        .release(middle, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()
}

#[when(
    regex = r#"^the person clicks the button "([^"]+)" and the preview's answer about "([^"]+)" comes first, saying "([^"]+)"$"#
)]
fn late_answer(w: &mut UiWorld, button: String, about: String, says: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let node = find(&tree, &|n| n.name == button && n.role == "button")
        .cloned()
        .ok_or_else(|| format!("no button {button:?}; the window holds {}", names(&tree)))?;
    let key = node.key.ok_or("the button has no key")?;
    let name = button
        .strip_prefix("Pick ")
        .ok_or("not a picker's button")?;
    let mut driver = window.stage.drive();
    let at = driver.aim_at_node(key).map_err(|why| format!("{why:?}"))?;
    driver
        .click(at, MouseButtons::LEFT, 1)
        .map_err(drive_error)?;
    let picker = ljos_drive::find_below(window.engine(), window.root, "colourPicker")
        .ok_or("the window has no colour picker")?;
    // The driver's click waits for the real answer, so the picker is put
    // back to waiting for the late one to find it as it would have.
    window
        .engine()
        .write_by_name(picker, "waiting", Value::Bool(true))
        .map_err(|e| format!("{e:?}"))?;
    let answer = serde_json::json!({"id": about, "state": {name: says}, "mocked": false});
    let asked = serde_json::json!({"node": {"node": about}, "read": [name]});
    window.answer("preview_state", answer, asked)?;
    window.rest()
}

/// The picker on `button` opened and waiting, as `late_answer`'s is; what it
/// asked about, for an answer to name.
fn picker_waiting(window: &mut Window, button: &str) -> Result<(String, String), String> {
    let tree = window.tree();
    let node = find(&tree, &|n| n.name == button && n.role == "button")
        .cloned()
        .ok_or_else(|| format!("no button {button:?}; the window holds {}", names(&tree)))?;
    let key = node.key.ok_or("the button has no key")?;
    let name = button
        .strip_prefix("Pick ")
        .ok_or("not a picker's button")?;
    let mut driver = window.stage.drive();
    let at = driver.aim_at_node(key).map_err(|why| format!("{why:?}"))?;
    driver
        .click(at, MouseButtons::LEFT, 1)
        .map_err(drive_error)?;
    let picker = ljos_drive::find_below(window.engine(), window.root, "colourPicker")
        .ok_or("the window has no colour picker")?;
    window
        .engine()
        .write_by_name(picker, "waiting", Value::Bool(true))
        .map_err(|e| format!("{e:?}"))?;
    let Value::String(on) = window.engine().read_by_name(picker, "node") else {
        return Err("the picker is on no object".into());
    };
    Ok((name.to_owned(), on.to_string()))
}

#[when(
    regex = r#"^the person clicks the button "([^"]+)" and the answer to showing a state on the same object comes first$"#
)]
fn other_call_answer(w: &mut UiWorld, button: String) -> StepResult {
    let window = w.window()?;
    let (_, on) = picker_waiting(window, &button)?;
    let answer = serde_json::json!({"id": "a", "state": {}, "mocked": false});
    let asked = serde_json::json!({"node": {"node": on}, "show": "open"});
    // No rest: the read this picker really sent is on its way, and a preview
    // slow enough answers it inside one, which rightly ends the wait.
    window.answer("preview_state", answer, asked)
}

#[when(
    regex = r#"^the person clicks the button "([^"]+)" and the preview's answer has no value for it$"#
)]
fn empty_answer(w: &mut UiWorld, button: String) -> StepResult {
    let window = w.window()?;
    let (name, on) = picker_waiting(window, &button)?;
    let answer = serde_json::json!({"id": "a", "state": {}, "mocked": false});
    let asked = serde_json::json!({"node": {"node": on}, "read": [name]});
    window.answer("preview_state", answer, asked)?;
    window.rest()
}

#[then("the colour picker still waits for the preview")]
fn picker_still_waits(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let picker = ljos_drive::find_below(window.engine(), window.root, "colourPicker")
        .ok_or("the window has no colour picker")?;
    (window.engine().read_by_name(picker, "waiting") == Value::Bool(true))
        .then_some(())
        .ok_or_else(|| "another call's answer ended the picker's wait".to_owned())
}

#[then(regex = r#"^the window has no colour picker open$"#)]
fn no_picker_open(w: &mut UiWorld) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == "Colour")
        .is_none()
        .then_some(())
        .ok_or_else(|| "the colour picker is still open".to_owned())
}

#[then(regex = r#"^the picker's "([^"]+)" can be moved$"#)]
fn picker_slider_enabled(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let picker = ljos_drive::find_below(window.engine(), window.root, "colourPicker")
        .ok_or("the window has no colour picker")?;
    (window.engine().read_by_name(picker, "known") == Value::Bool(true))
        .then_some(())
        .ok_or_else(|| format!("the picker's {name:?} stays still: it has no colour to start from"))
}

#[when(regex = r#"^the person drags the picker's "([^"]+)" to its middle$"#)]
fn drags_picker_slider(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let slider = find(&tree, &|n| n.name == name && n.role == "slider")
        .cloned()
        .ok_or_else(|| format!("no slider {name:?}; the window holds {}", names(&tree)))?;
    let key = slider.key.ok_or("the slider has no key")?;
    let mut driver = window.stage.drive();
    let middle = driver.aim_at_node(key).map_err(|why| format!("{why:?}"))?;
    driver
        .press(middle, MouseButtons::LEFT)
        .map_err(drive_error)?;
    // Past the threshold, as a hand goes: the end is the slider's middle.
    let start = Point::new(middle.x - 12.0, middle.y);
    driver.move_to(start).map_err(drive_error)?;
    driver.move_to(middle).map_err(drive_error)?;
    driver
        .release(middle, MouseButtons::LEFT)
        .map_err(drive_error)?;
    window.rest()
}

#[then(regex = r#"^the colour picker waits on nothing$"#)]
fn picker_waits_on_nothing(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let picker = ljos_drive::find_below(window.engine(), window.root, "colourPicker")
        .ok_or("the window has no colour picker")?;
    (window.engine().read_by_name(picker, "waiting") == Value::Bool(false))
        .then_some(())
        .ok_or_else(|| "the colour picker still waits for the preview".to_owned())
}

#[then(regex = r#"^the colour picker holds "([^"]+)"$"#)]
fn picker_holds(w: &mut UiWorld, want: String) -> StepResult {
    let tree = w.window()?.tree();
    let field = find(&tree, &|n| n.name == "Colour" && n.key.is_some())
        .cloned()
        .ok_or_else(|| format!("no colour field; the window holds {}", names(&tree)))?;
    (field.value == want)
        .then_some(())
        .ok_or_else(|| format!("the colour picker holds {:?}", field.value))
}

#[then(regex = r#"^the layers panel lists "([^"]*)"$"#)]
fn layers_list(w: &mut UiWorld, want: String) -> StepResult {
    let window = w.window()?;
    let want = list(&want);
    let shown: Vec<String> = window.shown_layers()?.into_iter().map(|n| n.name).collect();
    (shown == want)
        .then_some(())
        .ok_or_else(|| format!("the layers panel lists {shown:?}"))
}

#[then(regex = r#"^the layers panel names no object$"#)]
fn names_no_object(w: &mut UiWorld) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name.starts_with("Name of "))
        .map_or(Ok(()), |n| Err(format!("the field {:?} is open", n.name)))
}

#[then(regex = r#"^the layers panel is naming "([^"]+)"$"#)]
fn naming(w: &mut UiWorld, label: String) -> StepResult {
    let tree = w.window()?.tree();
    let name = format!("Name of {label}");
    find(&tree, &|n| n.name == name)
        .map(|_| ())
        .ok_or_else(|| format!("the field {name:?} is not open"))
}

#[then(regex = r#"^the layers panel has "([^"]+)" selected$"#)]
fn layer_selected(w: &mut UiWorld, label: String) -> StepResult {
    let window = w.window()?;
    let rows = window.shown_layers()?;
    let highlighted: Vec<String> = rows
        .iter()
        .filter(|n| {
            n.key
                .and_then(|k| window.stage.scene.accessible_item(k))
                .is_some_and(|item| {
                    window.engine().read_by_name(item, "highlighted") == Value::Bool(true)
                })
        })
        .map(|n| n.name.clone())
        .collect();
    (highlighted == [label.clone()])
        .then_some(())
        .ok_or_else(|| format!("the layers panel has {highlighted:?} selected"))
}

#[then(regex = r#"^the code view shows "([^"]+)"$"#)]
fn code_shows(w: &mut UiWorld, doc: String) -> StepResult {
    let text = std::fs::read_to_string(w.dir.path().join(&doc)).map_err(|e| e.to_string())?;
    let window = w.window()?;
    let code = window.code()?;
    if code.value != text {
        return Err(format!("the code view shows:\n{}", code.value));
    }
    (code.name.contains(&doc))
        .then_some(())
        .ok_or_else(|| format!("the code view is called {:?}", code.name))
}

#[then(regex = r#"^the code view selects "(.*)"$"#)]
fn code_selects(w: &mut UiWorld, want: String) -> StepResult {
    let want = unescape(&want);
    let window = w.window()?;
    let code = window.code()?;
    (code.selected == want)
        .then_some(())
        .ok_or_else(|| format!("the code view selects {:?}", code.selected))
}

#[then(regex = r#"^the window shows the document "([^"]+)"$"#)]
fn window_shows_document(w: &mut UiWorld, want: String) -> StepResult {
    let doc = w.window()?.read("doc").to_js_string().to_string();
    (doc == want)
        .then_some(())
        .ok_or_else(|| format!("the window shows the document {doc:?}"))
}

#[then("the documents panel says why it made no document")]
fn documents_says_why(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let label = ljos_drive::find_below(window.engine(), window.root, "newDocumentRefusal")
        .ok_or("the documents panel has no place for a refusal")?;
    let text = window
        .engine()
        .read_by_name(label, "text")
        .to_js_string()
        .to_string();
    (!text.is_empty())
        .then_some(())
        .ok_or_else(|| "the documents panel says nothing".to_owned())
}

#[then(regex = r#"^the code view marks "(.*)"$"#)]
fn code_marks(w: &mut UiWorld, want: String) -> StepResult {
    let want = unescape(&want);
    let window = w.window()?;
    let band = ljos_drive::find_below(window.engine(), window.root, "codeBand")
        .ok_or("the code view has no band")?;
    let engine = window.engine();
    if engine.read_by_name(band, "visible") != Value::Bool(true) {
        return Err("the band is not shown".to_owned());
    }
    let marks = engine.read_by_name(band, "marks");
    (marks == Value::from(want.as_str()))
        .then_some(())
        .ok_or_else(|| format!("the code view marks {marks:?}"))
}

#[then(regex = r#"^the inspector shows "([^"]+)" as "(.*)"$"#)]
fn inspector_shows(w: &mut UiWorld, name: String, value: String) -> StepResult {
    let value = unescape(&value);
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), &name)?;
    (field.value == value)
        .then_some(())
        .ok_or_else(|| format!("the inspector shows {name} as {:?}", field.value))
}

#[then(regex = r#"^the inspector shows the (open|resolved|orphaned) comment "(.*)"$"#)]
fn inspector_shows_comment(w: &mut UiWorld, kind: String, text: String) -> StepResult {
    let window = w.window()?;
    let mut kind = kind;
    kind[..1].make_ascii_uppercase();
    window
        .node(
            "Inspector",
            None,
            &format!("{kind} comment: {}", unescape(&text)),
        )
        .map(|_| ())
}

/// The layers' comment counts: what the panel shows beside the rows.
/// What the layers panel says of its layers beside their names: how many
/// comments are on one, who holds one.
fn comment_counts(window: &Window) -> Result<Vec<String>, String> {
    let tree = window.tree();
    let list = find(&tree, &|n| n.name == "Layers" && n.role == "list")
        .ok_or("no layers list in the window")?;
    let mut counts = Vec::new();
    collect(list, &mut |n| {
        // Static text reads as a label.
        if n.key.is_some()
            && n.role == "label"
            && (n.name.contains(" comment") || n.name.contains(" held by "))
        {
            counts.push(n.name.clone());
        }
    });
    Ok(counts)
}

#[then(regex = r#"^the layers panel shows "([^"]+)"$"#)]
fn layers_show(w: &mut UiWorld, want: String) -> StepResult {
    let counts = comment_counts(w.window()?)?;
    (counts == [want])
        .then_some(())
        .ok_or_else(|| format!("the layers panel shows {counts:?}"))
}

#[then("the layers panel shows no comments")]
fn layers_show_no_comments(w: &mut UiWorld) -> StepResult {
    let counts = comment_counts(w.window()?)?;
    counts
        .is_empty()
        .then_some(())
        .ok_or_else(|| format!("the layers panel shows {counts:?}"))
}

#[then(regex = r#"^the file "([^"]+)" is:$"#)]
fn file_is(w: &mut UiWorld, name: String, step: &Step) -> StepResult {
    let want = file_text(step)?;
    let got =
        std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| format!("`{name}`: {e}"))?;
    (got == want).then_some(()).ok_or_else(|| {
        let shown = w.window.as_ref().map(|window| {
            format!(
                "\nthe window's history is {} then {}, its last refusal {:?}",
                window.read_json("done"),
                window.read_json("undone"),
                window.read("error").to_js_string()
            )
        });
        format!(
            "`{name}` is:\n{got}\nnot:\n{want}{}",
            shown.unwrap_or_default()
        )
    })
}

#[given(regex = r#"^the person has set the output directory to "([^"]*)" in the export panel$"#)]
#[when(regex = r#"^the person sets the output directory to "([^"]*)" in the export panel$"#)]
fn sets_export_out(w: &mut UiWorld, out: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Export panel", Some("textinput"), "Output directory")?;
    window.enter(&field, &out)
}

// No Return: what is typed is checked once typing pauses.
#[when(regex = r#"^the person types the output directory "([^"]*)" in the export panel$"#)]
fn types_export_out(w: &mut UiWorld, out: String) -> StepResult {
    goes_on_typing_export_out(w, out)?;
    export_typing_pauses(w)
}

#[when(regex = r#"^the person is typing the output directory "([^"]*)" in the export panel$"#)]
fn goes_on_typing_export_out(w: &mut UiWorld, out: String) -> StepResult {
    let window = w.window()?;
    let field = window.node("Export panel", Some("textinput"), "Output directory")?;
    window.click_node(&field)?;
    let mut driver = window.stage.drive();
    driver
        .chord(&Chord::parse("Ctrl+A")?, 0)
        .map_err(drive_error)?;
    driver.commit(&out).map_err(drive_error)?;
    window.rest()
}

#[when("the person pauses typing in the export panel")]
fn export_typing_pauses(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    window.advance(300)?;
    window.rest()
}

#[then(regex = r#"^the export panel does not say "(.+)"$"#)]
fn export_does_not_say(w: &mut UiWorld, pattern: String) -> StepResult {
    let pattern = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
    let window = w.window()?;
    let tree = window.tree();
    let panel = find(&tree, &|n| n.name == "Export panel" && n.role != "label")
        .ok_or("no export panel in the window")?;
    match find(panel, &|n| pattern.is_match(&n.name)) {
        Some(n) => Err(format!("the export panel says {:?}", n.name)),
        None => Ok(()),
    }
}

#[when(regex = r#"^the file "([^"]+)" is written outside Studio:$"#)]
fn written_outside(w: &mut UiWorld, name: String, step: &Step) -> StepResult {
    the_file(w, name, step)
}

/// Lets the window run until `shown` holds for its documents panel. The
/// report comes from another thread on the real clock.
fn documents_until(w: &mut UiWorld, seconds: u64, name: &str, shown: bool) -> StepResult {
    let window = w.window()?;
    for _ in 0..seconds * 20 {
        if window.node("Documents", None, name).is_ok() == shown {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        window.stage.drive().advance(50).map_err(drive_error)?;
        window.rest()?;
    }
    Err(format!(
        "after {seconds} seconds the documents panel still {} {name:?}",
        if shown { "lacks" } else { "lists" }
    ))
}

#[then(regex = r#"^the documents panel lists "([^"]+)" within (\d+) seconds$"#)]
fn documents_lists(w: &mut UiWorld, name: String, seconds: u64) -> StepResult {
    documents_until(w, seconds, &name, true)
}

#[then(regex = r#"^the documents panel does not list "([^"]+)" within (\d+) seconds$"#)]
fn documents_lists_not(w: &mut UiWorld, name: String, seconds: u64) -> StepResult {
    documents_until(w, seconds, &name, false)
}

#[when(regex = r#"^the file "([^"]+)" is deleted$"#)]
fn file_deleted(w: &mut UiWorld, name: String) -> StepResult {
    std::fs::remove_file(w.dir.path().join(name)).map_err(|e| e.to_string())
}

// Long names, so few enough files pass the size an answer may be.
#[given(regex = r"^(\d+) files the bundle holds$")]
fn bundle_files(w: &mut UiWorld, count: usize) -> StepResult {
    let dir = w.dir.path().join("assets");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for n in 0..count {
        let name = format!("an-asset-with-a-name-long-enough-to-count-{n:04}.svg");
        std::fs::write(dir.join(name), "<svg/>").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[when("the person pastes into the output directory in the export panel")]
fn pastes_export_out(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let field = window.node("Export panel", Some("textinput"), "Output directory")?;
    window.click_node(&field)?;
    window
        .stage
        .drive()
        .chord(&Chord::parse("Ctrl+V")?, 0)
        .map_err(drive_error)?;
    window.rest()
}

#[then(regex = r#"^the export panel's output directory reads "(.+)"$"#)]
fn export_out_reads(w: &mut UiWorld, pattern: String) -> StepResult {
    let pattern = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
    let field = w
        .window()?
        .node("Export panel", Some("textinput"), "Output directory")?;
    pattern
        .is_match(&field.value)
        .then_some(())
        .ok_or_else(|| format!("it reads {:?}", field.value))
}

#[when(regex = r#"^the person chooses the entry "([^"]+)" in the export panel$"#)]
fn chooses_export_entry(w: &mut UiWorld, entry: String) -> StepResult {
    let window = w.window()?;
    let combo = window.node("Export panel", None, "Entry document")?;
    window.click_node(&combo)?;
    let tree = window.tree();
    let mut rows = Vec::new();
    collect_named(&tree, &entry, &mut rows);
    // The popup is drawn over everything, so it is read last.
    let row = rows
        .last()
        .cloned()
        .ok_or_else(|| format!("no row {entry:?}; the window holds {}", names(&tree)))?;
    window.click_node(&row)
}

fn collect_named(node: &AccessibleNode, name: &str, rows: &mut Vec<AccessibleNode>) {
    if node.name == name && node.key.is_some() {
        rows.push(node.clone());
    }
    for child in &node.children {
        collect_named(child, name, rows);
    }
}

#[given(regex = r#"^the person has chosen "([^"]+)" in the export panel$"#)]
#[when(regex = r#"^the person chooses "([^"]+)" in the export panel$"#)]
fn chooses_export_kind(w: &mut UiWorld, kind: String) -> StepResult {
    let window = w.window()?;
    let radio = window.node("Export panel", Some("radiobutton"), &kind)?;
    window.click_node(&radio)
}

#[then(regex = r#"^the export panel says "(.+)"$"#)]
fn export_says(w: &mut UiWorld, pattern: String) -> StepResult {
    let pattern = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
    let window = w.window()?;
    let tree = window.tree();
    let panel = find(&tree, &|n| n.name == "Export panel" && n.role != "label")
        .ok_or("no export panel in the window")?;
    find(panel, &|n| pattern.is_match(&n.name))
        .map(drop)
        .ok_or_else(|| {
            format!(
                "the export panel says none of {}; the last refusal {:?}",
                names(panel),
                window.read("error").to_js_string()
            )
        })
}

#[then("the export panel is closed")]
fn export_closed(w: &mut UiWorld) -> StepResult {
    let tree = w.window()?.tree();
    match find(&tree, &|n| n.name == "Export panel" && n.role != "label") {
        None => Ok(()),
        Some(_) => Err("the export panel is still in the window".to_owned()),
    }
}

#[then(regex = r#"^the export panel's button "([^"]+)" is (enabled|disabled)$"#)]
fn export_button(w: &mut UiWorld, name: String, state: String) -> StepResult {
    let button = w.window()?.node("Export panel", Some("button"), &name)?;
    let disabled = button.states.iter().any(|s| s == "disabled");
    (disabled == (state == "disabled"))
        .then_some(())
        .ok_or_else(|| format!("{name:?} is {:?}", button.states))
}

#[then(regex = r#"^the file "([^"]+)" exists$"#)]
fn file_present(w: &mut UiWorld, name: String) -> StepResult {
    let path = w.dir.path().join(&name);
    path.is_file()
        .then_some(())
        .ok_or_else(|| format!("`{name}` is not a file"))
}

#[then(regex = r#"^the file "([^"]+)" does not exist$"#)]
fn file_absent(w: &mut UiWorld, name: String) -> StepResult {
    let path = w.dir.path().join(&name);
    match std::fs::read_to_string(&path) {
        Ok(text) => Err(format!("`{name}` is:\n{text}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("`{name}`: {e}")),
    }
}

#[then("the history lists nothing")]
fn history_lists_nothing(w: &mut UiWorld) -> StepResult {
    let done = w.window()?.read_json("done");
    done.as_array()
        .is_some_and(Vec::is_empty)
        .then_some(())
        .ok_or_else(|| format!("the history lists {done}"))
}

#[then(regex = r#"^the history lists "([^"]+)"$"#)]
fn history_lists(w: &mut UiWorld, label: String) -> StepResult {
    let window = w.window()?;
    let done = window.read_json("done");
    done.as_array()
        .is_some_and(|d| d.iter().any(|l| l == label.as_str()))
        .then_some(())
        .ok_or_else(|| format!("the history lists {done}"))
}

#[then(regex = r#"^the canvas has a frame "([^"]+)" of (\d+) by (\d+)$"#)]
fn frame_of(w: &mut UiWorld, target: String, width: u32, height: u32) -> StepResult {
    let window = w.window()?;
    let frame = window.frame(&target)?;
    let label = ljos_drive::find_below(window.engine(), frame, "frameLabel")
        .ok_or("the frame has no label")?;
    let text = window
        .engine()
        .read_by_name(label, "text")
        .to_js_string()
        .to_string();
    let want = format!("{target} · {width} × {height}");
    (text == want)
        .then_some(())
        .ok_or_else(|| format!("the frame is labelled {text:?}"))
}

fn picture_drawn(window: &Window, target: &str) -> Result<u64, String> {
    let frame = window.frame(target)?;
    let picture = ljos_drive::find_below(window.engine(), frame, "picture")
        .ok_or("the frame has no picture")?;
    let node = window
        .stage
        .scene
        .node_of(picture)
        .ok_or("the picture is not shown")?;
    let drew = window
        .stage
        .scene
        .render_tree()
        .node(node)
        .and_then(|n| n.content())
        .is_some_and(|c| c.ops().iter().any(|op| matches!(op, DrawOp::Image { .. })));
    if !drew {
        return Err(format!(
            "the frame draws no picture; the preview is {}, the last error {:?}",
            window.read_json("preview"),
            window.read("error").to_js_string()
        ));
    }
    Ok(window.read_json("preview")["picture"].as_u64().unwrap_or(0))
}

#[then(regex = r#"^the frame "([^"]+)"'s picture is red at (\d+), (\d+)$"#)]
fn frame_red_at(w: &mut UiWorld, target: String, x: u32, y: u32) -> StepResult {
    let window = w.window()?;
    let frame = window.frame(&target)?;
    let picture = ljos_drive::find_below(window.engine(), frame, "picture")
        .ok_or("the frame has no picture")?;
    let node = window
        .stage
        .scene
        .node_of(picture)
        .ok_or("the picture is not shown")?;
    let tree = window.stage.scene.render_tree();
    let image = tree
        .node(node)
        .and_then(|n| n.content())
        .and_then(|c| {
            c.ops().iter().find_map(|op| match op {
                DrawOp::Image { image, .. } => Some(image.clone()),
                _ => None,
            })
        })
        .ok_or("the frame draws no picture")?;
    let [r, g, b, _] = image.pixel(x, y);
    (r > 200 && g < 80 && b < 80)
        .then_some(())
        .ok_or_else(|| format!("the picture is rgb({r},{g},{b}) there"))
}

#[then(regex = r#"^the frame "([^"]+)" shows a picture of the preview$"#)]
fn frame_shows_picture(w: &mut UiWorld, target: String) -> StepResult {
    picture_drawn(w.window()?, &target).map(drop)
}

#[given(regex = r#"^the person has looked at the frame "([^"]+)"$"#)]
fn looks_at_frame(w: &mut UiWorld, target: String) -> StepResult {
    let window = w.window()?;
    picture_drawn(window, &target)?;
    let shots = window.read_json("preview")["shots"].as_u64().unwrap_or(0);
    w.looked = shots;
    Ok(())
}

#[then(regex = r#"^the frame "([^"]+)" has drawn a different picture since$"#)]
fn frame_drew_another(w: &mut UiWorld, target: String) -> StepResult {
    let looked = w.looked;
    let window = w.window()?;
    // The app is another process on the real clock, the window's timers on
    // the driver's: both have to move. Studio stops looking by itself.
    while window.read("settling") == Value::Bool(true) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        window.advance(100)?;
        window.rest()?;
    }
    picture_drawn(window, &target)?;
    let preview = window.read_json("preview");
    (preview["shots"].as_u64().unwrap_or(0) > looked)
        .then_some(())
        .ok_or_else(|| format!("the frame still draws the picture it had; {preview}"))
}

/// The app is another process on the real clock, the window's timers on the
/// driver's: both have to move.
fn let_pass(w: &mut UiWorld, seconds: u64) -> StepResult {
    let window = w.window()?;
    for _ in 0..seconds * 10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        window.advance(100)?;
        window.rest()?;
    }
    Ok(())
}

#[given(regex = r"^(\d+) seconds have passed$")]
fn seconds_have_passed(w: &mut UiWorld, seconds: u64) -> StepResult {
    let_pass(w, seconds)
}

#[when(regex = r"^(\d+) seconds pass$")]
fn seconds_pass(w: &mut UiWorld, seconds: u64) -> StepResult {
    let_pass(w, seconds)
}

#[given(regex = r"^the person has counted the pictures asked for$")]
fn counts_pictures(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    w.asked = window.read_json("looks").as_u64().unwrap_or(0);
    Ok(())
}

#[then(regex = r"^no more pictures have been asked for$")]
fn no_more_pictures(w: &mut UiWorld) -> StepResult {
    let asked = w.asked;
    let now = w.window()?.read_json("looks").as_u64().unwrap_or(0);
    (now == asked)
        .then_some(())
        .ok_or_else(|| format!("{} pictures asked for since", now - asked))
}

#[then(regex = r#"^the frame "([^"]+)" shows the preview's generation (\d+)$"#)]
fn frame_shows_generation(w: &mut UiWorld, target: String, generation: u64) -> StepResult {
    let shown = picture_drawn(w.window()?, &target)?;
    (shown == generation)
        .then_some(())
        .ok_or_else(|| format!("the frame shows the preview's generation {shown}"))
}

#[then("Studio's window ran no frames")]
fn ran_no_frames(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let idle = window.idle.take().ok_or("nobody left the window alone")?;
    if idle.wake_before.is_some() || idle.wake_after.is_some() {
        return Err(format!(
            "the window asks for a frame at {:?} and then at {:?}",
            idle.wake_before, idle.wake_after
        ));
    }
    (idle.shown == 0)
        .then_some(())
        .ok_or_else(|| format!("the window showed {} new frames", idle.shown))
}

#[given("the preview does not say where its objects are")]
fn no_places(w: &mut UiWorld) {
    w.no_places = true;
}

#[given(regex = r"^the preview takes (\d+) ms over each picture$")]
fn slow_pictures(w: &mut UiWorld, ms: u64) {
    w.picture_ms.store(ms, Ordering::SeqCst);
}

#[given("Studio's window runs the person's shell, a copy of its own")]
fn runs_own_shell(w: &mut UiWorld) -> StepResult {
    studio_blocks::shell::seed(w.dir.path()).map_err(|e| e.to_string())?;
    w.open_watched()
}

#[given(regex = r#"^the person's "([^"]+)" has "(.*)" replaced with "(.*)"$"#)]
fn file_replaced(w: &mut UiWorld, file: String, old: String, new: String) -> StepResult {
    let (old, new) = (unescape(&old), unescape(&new));
    let path = w.dir.path().join(&file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
    if !text.contains(&old) {
        return Err(format!("{file} does not have {old:?}"));
    }
    std::fs::write(&path, text.replacen(&old, &new, 1)).map_err(|e| format!("{file}: {e}"))
}

#[when("Studio's window opens again")]
fn opens_again(w: &mut UiWorld) -> StepResult {
    // The old window, its watcher first, goes before the new one watches
    // the same files.
    w.window = None;
    w.open_watched()
}

/// The ids of the threads following a preview's picture, as the operating
/// system lists them: what a person's task manager shows, so there is no
/// hook the product keeps for a test. An earlier scenario's window may be
/// ending its own, so a scenario knows its thread by being new.
fn picture_followers() -> Result<Vec<String>, String> {
    // A thread's name is cut to 15 bytes there.
    const NAME: &str = "studio-picture-";
    let tasks =
        std::fs::read_dir("/proc/self/task").map_err(|e| format!("/proc/self/task: {e}"))?;
    Ok(tasks
        .filter_map(Result::ok)
        .filter(|t| {
            std::fs::read_to_string(t.path().join("comm")).is_ok_and(|name| name.starts_with(NAME))
        })
        .map(|t| t.file_name().to_string_lossy().into_owned())
        .collect())
}

#[given("the person has listed the threads following the preview's picture")]
fn list_followers(w: &mut UiWorld) -> StepResult {
    w.followers_before = picture_followers()?;
    Ok(())
}

#[given("a new thread is following the preview's picture")]
fn new_follower(w: &mut UiWorld) -> StepResult {
    let before = w.followers_before.clone();
    let window = w.window()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let new: Vec<String> = picture_followers()?
            .into_iter()
            .filter(|t| !before.contains(t))
            .collect();
        match new.as_slice() {
            [one] => {
                w.follower = Some(one.clone());
                return Ok(());
            }
            _ if std::time::Instant::now() >= deadline => {
                return Err(format!("{} new threads follow the picture", new.len()));
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(50));
        window.stage.drive().advance(50).map_err(drive_error)?;
    }
}

// The seconds are real ones: a thread that sees it should stop only when its
// wait on the preview ends, which for an app at rest is half a minute, is not
// counted out in these.
#[then(regex = r"^that thread has ended within (\d+) seconds?$")]
fn follower_ended(w: &mut UiWorld, seconds: u64) -> StepResult {
    let thread = w.follower.clone().ok_or("no thread was told")?;
    let window = w.window()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        if !picture_followers()?.contains(&thread) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "the thread following the picture is still there after {seconds} s"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
        window.stage.drive().advance(50).map_err(drive_error)?;
    }
}

#[given("the person has remembered the window's workspace")]
fn remembers_workspace(w: &mut UiWorld) -> StepResult {
    w.remembered = Some(w.window()?.workspace);
    Ok(())
}

#[then("the remembered workspace is gone")]
fn remembered_is_gone(w: &mut UiWorld) -> StepResult {
    let was = w.remembered.ok_or("no workspace was remembered")?;
    (!w.window()?.engine().is_alive(was))
        .then_some(())
        .ok_or_else(|| "the reload kept the workspace, so this says nothing".to_owned())
}

#[given("the window has taken the change")]
#[when("the window takes the change")]
fn takes_change(w: &mut UiWorld) -> StepResult {
    w.window()?.take_change()
}

#[then("the window shows the person's shell")]
fn shows_own_shell(w: &mut UiWorld) -> StepResult {
    shows_shell(w, ShellStatus::Ready)
}

#[then("the window shows the built-in shell")]
fn shows_builtin_shell(w: &mut UiWorld) -> StepResult {
    shows_shell(w, ShellStatus::Fallback)
}

fn shows_shell(w: &mut UiWorld, want: ShellStatus) -> StepResult {
    let window = w.window()?;
    let status = window.loader_status()?;
    if status == want {
        return Ok(());
    }
    Err(format!(
        "the shell loader is {status:?}, not {want:?}; its error is {:?}, the banner {:?}",
        window.loader_read("error").to_js_string(),
        window.banner()?
    ))
}

#[then("the window has no shell banner")]
fn no_banner(w: &mut UiWorld) -> StepResult {
    match w.window()?.banner()? {
        said if said.is_empty() => Ok(()),
        said => Err(format!("the shell banner says {said:?}")),
    }
}

#[then(regex = r#"^the shell banner says "(.+)"$"#)]
fn banner_says(w: &mut UiWorld, pattern: String) -> StepResult {
    let pattern = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
    let window = w.window()?;
    let said = window.banner()?;
    if !pattern.is_match(&said) {
        return Err(format!("the shell banner says {said:?}"));
    }
    // An assistive technology hears it as an alert, by the same words.
    let tree = window.tree();
    let alert = find(&tree, &|n| n.name == "Shell error").ok_or_else(|| {
        format!(
            "no \"Shell error\" in the window; it holds {}",
            names(&tree)
        )
    })?;
    if alert.description != said {
        return Err(format!(
            "the banner is heard as {:?}, not {said:?}",
            alert.description
        ));
    }
    Ok(())
}

#[then(regex = r#"^the window's canvas is "(#[0-9a-fA-F]{6})"$"#)]
fn canvas_is(w: &mut UiWorld, colour: String) -> StepResult {
    let window = w.window()?;
    let shown = window
        .engine()
        .read_by_name(window.root, "color")
        .to_js_string();
    if shown.eq_ignore_ascii_case(&colour) {
        return Ok(());
    }
    Err(format!("the window's canvas is {shown}"))
}

#[then(regex = r#"^the window has a pane "([^"]+)"$"#)]
fn has_pane(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    if find(&tree, &|n| n.name == name && n.role == "pane").is_some() {
        return Ok(());
    }
    Err(format!(
        "no pane {name:?} in the window; it holds {}",
        names(&tree)
    ))
}

/// `STUDIO_PERF_EXPORT=hello|200`: times the export panel's opening, from the
/// click on the toolbar's button to the list it shows, on `samples/hello` or
/// on a project of 200 files, in place of the scenarios. Run it in release.
fn perf_export(project: &str) {
    let mut w = UiWorld::new();
    let root = w.dir.path().to_owned();
    if project == "hello" {
        let hello = format!("{}/../../samples/hello/Main.qml", manifest_dir());
        std::fs::copy(hello, root.join("Main.qml")).expect("samples/hello");
    } else {
        std::fs::write(root.join("Main.qml"), "import QtQuick\n\nItem {}\n").unwrap();
        for d in 0..199 {
            let mut text = String::from("import QtQuick\n\nColumn {\n");
            for i in 0..40 {
                text += &format!("    Rectangle {{ id: r{i}; width: {i}; height: 10 }}\n");
            }
            text += "}\n";
            std::fs::write(root.join(format!("S{d}.qml")), text).unwrap();
        }
    }
    w.open().expect("the window opens");
    let mut samples = Vec::new();
    for _ in 0..20 {
        let window = w.window().unwrap();
        let button = window
            .node("Export", Some("button"), "Export")
            .or_else(|_| {
                let tree = window.tree();
                find(&tree, &|n| {
                    n.name == "Export" && n.role == "button" && n.key.is_some()
                })
                .cloned()
                .ok_or_else(|| "no Export button".to_owned())
            })
            .expect("the toolbar's button");
        let started = std::time::Instant::now();
        window.click_node(&button).expect("opens");
        samples.push(started.elapsed());
        let tree = window.tree();
        assert!(
            find(&tree, &|n| n.name.starts_with("Would write")).is_some(),
            "the panel shows no list: {}",
            names(&tree)
        );
        let escape = Chord::parse("Escape").unwrap();
        window.stage.drive().chord(&escape, 0).unwrap();
        window.rest().unwrap();
    }
    samples.sort();
    let at = |p: f64| samples[((samples.len() - 1) as f64 * p).round() as usize];
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "export panel open to list, {project}: p50 {:.2} ms, p95 {:.2} ms over {}",
        ms(at(0.5)),
        ms(at(0.95)),
        samples.len()
    );
}

/// Told each scenario as it starts. One that outlasts its patience ends the
/// run with its name: nothing on the Engine's thread can, if that is stuck.
fn scenario_watch() -> mpsc::Sender<String> {
    let (tell, told) = mpsc::channel();
    std::thread::spawn(move || {
        let mut running = String::new();
        loop {
            match told.recv_timeout(SCENARIO_PATIENCE) {
                Ok(name) => running = name,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!("the scenario {running:?} did not end in {SCENARIO_PATIENCE:?}");
                    std::process::exit(3);
                }
            }
        }
    });
    tell
}

fn main() {
    if let Ok(project) = std::env::var("STUDIO_PERF_EXPORT") {
        perf_export(&project);
        return;
    }
    let features = format!("{}/../../features/ui", manifest_dir());
    let input = std::env::var("STUDIO_FEATURES").unwrap_or(features);
    // The Engine lives on one thread: one scenario at a time, on this one.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let watch = scenario_watch();
    runtime.block_on(
        UiWorld::cucumber()
            .max_concurrent_scenarios(1)
            .fail_on_skipped()
            .before(move |_, _, scenario, _| {
                let _ = watch.send(scenario.name.clone());
                Box::pin(async {})
            })
            // `@web` windows are the shell compiled by ljos-web, driven in a
            // browser (`mise run web:shell`), not this window.
            .filter_run_and_exit(input, |feature, _, scenario| {
                let mut tags = feature.tags.iter().chain(&scenario.tags);
                tags.clone().any(|t| t == "ui") && !tags.any(|t| t == "web")
            }),
    );
}

#[given(regex = r#"^the person has opened the "([^"]+) tab"$"#)]
#[when(regex = r#"^the person opens the "([^"]+) tab"$"#)]
fn opens_tab(w: &mut UiWorld, tab: String) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let name = format!("{tab} tab");
    let node = find(&tree, &|n| n.name == name && n.key.is_some())
        .cloned()
        .ok_or_else(|| format!("no {name:?}; the window holds {}", names(&tree)))?;
    window.click_node(&node)
}

#[then(regex = r#"^the window has no pane "([^"]+)"$"#)]
fn has_no_pane(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    match find(&tree, &|n| n.name == name && n.role == "pane") {
        None => Ok(()),
        Some(_) => Err(format!("the window still has a pane {name:?}")),
    }
}

#[then(regex = r#"^the window has a button "([^"]+)"$"#)]
fn has_button(w: &mut UiWorld, name: String) -> StepResult {
    let tree = w.window()?.tree();
    find(&tree, &|n| n.name == name && n.role == "button")
        .map(drop)
        .ok_or_else(|| format!("no button {name:?}; the window holds {}", names(&tree)))
}

/// Where the frame for `target` is drawn; its button and label share the name.
fn frame_bounds(window: &Window, target: &str) -> Result<ljos::item::kurbo::Rect, String> {
    window
        .bounds(&|role, name| name == target && role != "button" && role != "label")
        .ok_or_else(|| format!("no frame {target:?} drawn in the window"))
}

#[when("the person turns the wheel over the canvas")]
fn wheel_over_canvas(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let before = frame_bounds(window, "Desktop")?;
    window
        .stage
        .drive()
        .wheel(before.center(), Wheel::Notches(0.0, -1.0))
        .map_err(drive_error)?;
    window.stage.drive().advance(1000).map_err(drive_error)?;
    window.rest()?;
    w.frame_before = Some(before);
    Ok(())
}

#[when(
    regex = r#"^the person drags with the hand in the frame "([^"]+)" from (\d+), (\d+) by (\d+), (\d+)$"#
)]
fn hand_drag(w: &mut UiWorld, target: String, x: f64, y: f64, dx: f64, dy: f64) -> StepResult {
    let window = w.window()?;
    let before = frame_bounds(window, &target)?;
    let from = frame_point(window, &target, x, y)?;
    window
        .stage
        .drive()
        .drag(from, Point::new(from.x + dx, from.y + dy))
        .map_err(drive_error)?;
    window.rest()?;
    w.frame_before = Some(before);
    Ok(())
}

#[then(regex = r#"^the frame "([^"]+)" has moved by (\d+), (\d+)$"#)]
fn frame_moved_by(w: &mut UiWorld, target: String, dx: f64, dy: f64) -> StepResult {
    let before = w.frame_before.ok_or("no place noted")?;
    let now = frame_bounds(w.window()?, &target)?;
    if (now.x0 - before.x0 - dx).abs() > 1.0 || (now.y0 - before.y0 - dy).abs() > 1.0 {
        return Err(format!("the frame went from {before:?} to {now:?}"));
    }
    Ok(())
}

#[when(
    regex = r#"^the person zooms in with Ctrl and the wheel over the frame "([^"]+)" at (\d+), (\d+)$"#
)]
fn ctrl_wheel(w: &mut UiWorld, target: String, x: f64, y: f64) -> StepResult {
    let window = w.window()?;
    let before = frame_bounds(window, &target)?;
    let at = frame_point(window, &target, x, y)?;
    let mut driver = window.stage.drive();
    let mut turned = WheelEvent::notches(at.x, at.y, 0.0, 1.0, driver.now());
    turned.modifiers = ljos_drive::parse_modifiers("Ctrl")?;
    driver.send_wheel(&turned);
    driver.settle().map_err(drive_error)?;
    window.stage.drive().advance(1000).map_err(drive_error)?;
    window.rest()?;
    w.frame_before = Some(before);
    w.pointer_was = Some(at);
    Ok(())
}

#[then(regex = r#"^the frame "([^"]+)" grew, with its point under the pointer still there$"#)]
fn zoomed_about_pointer(w: &mut UiWorld, target: String) -> StepResult {
    let before = w.frame_before.ok_or("no place noted")?;
    let at = w.pointer_was.ok_or("no pointer noted")?;
    let now = frame_bounds(w.window()?, &target)?;
    if now.width() <= before.width() {
        return Err(format!("the frame went from {before:?} to {now:?}"));
    }
    let along =
        |r: &ljos::item::kurbo::Rect| ((at.x - r.x0) / r.width(), (at.y - r.y0) / r.height());
    let (was, is) = (along(&before), along(&now));
    if (was.0 - is.0).abs() > 0.01 || (was.1 - is.1).abs() > 0.01 {
        return Err(format!(
            "the point under the pointer went from {was:?} to {is:?} of the frame"
        ));
    }
    Ok(())
}

#[then(regex = r#"^the frame "([^"]+)" has moved$"#)]
fn frame_moved(w: &mut UiWorld, target: String) -> StepResult {
    let before = w.frame_before.ok_or("no place noted")?;
    let now = frame_bounds(w.window()?, &target)?;
    if (now.y0 - before.y0).abs() < 1.0 && (now.x0 - before.x0).abs() < 1.0 {
        return Err(format!("the frame stayed at {now:?}"));
    }
    Ok(())
}

#[then(regex = r#"^the frame "([^"]+)" has kept its size$"#)]
fn frame_kept_size(w: &mut UiWorld, target: String) -> StepResult {
    let before = w.frame_before.ok_or("no place noted")?;
    let now = frame_bounds(w.window()?, &target)?;
    if (now.width() - before.width()).abs() > 0.5 || (now.height() - before.height()).abs() > 0.5 {
        return Err(format!("the frame went from {before:?} to {now:?}"));
    }
    Ok(())
}

#[when("the person zooms in with the button")]
fn zooms_in(w: &mut UiWorld) -> StepResult {
    w.frame_before = Some(frame_bounds(w.window()?, "Desktop")?);
    clicks_button(w, "Zoom in".to_owned())
}

#[then(regex = r#"^the middle of the view shows the same place of the frame "([^"]+)"$"#)]
fn zoom_kept_middle(w: &mut UiWorld, target: String) -> StepResult {
    let before = w.frame_before.ok_or("no place noted")?;
    let window = w.window()?;
    let now = frame_bounds(window, &target)?;
    if now.width() <= before.width() {
        return Err(format!("the frame did not grow: {before:?} to {now:?}"));
    }
    let view = window
        .bounds(&|role, name| role == "pane" && name == "Canvas")
        .ok_or("no canvas pane")?
        .center();
    let place =
        |r: ljos::item::kurbo::Rect| ((view.x - r.x0) / r.width(), (view.y - r.y0) / r.height());
    let (was, is) = (place(before), place(now));
    if (was.0 - is.0).abs() > 0.01 || (was.1 - is.1).abs() > 0.01 {
        return Err(format!(
            "the view's middle was at {was:?} of the frame, now {is:?}"
        ));
    }
    Ok(())
}

#[when(regex = r"^the person zooms in with the button (\d+) times$")]
fn zooms_in_times(w: &mut UiWorld, times: u32) -> StepResult {
    w.frame_before = Some(frame_bounds(w.window()?, "Desktop")?);
    for _ in 0..times {
        clicks_button(w, "Zoom in".to_owned())?;
    }
    Ok(())
}

#[when("the person pans the canvas with the middle button")]
fn pans_with_middle(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let before = frame_bounds(window, "Desktop")?;
    let from = before.center();
    let to = Point::new(from.x + 40.0, from.y + 30.0);
    window
        .stage
        .drive()
        .press(from, MouseButtons::MIDDLE)
        .map_err(drive_error)?;
    window.rest()?;
    window.stage.drive().move_to(to).map_err(drive_error)?;
    window
        .stage
        .drive()
        .release(to, MouseButtons::MIDDLE)
        .map_err(drive_error)?;
    window.rest()?;
    w.frame_before = Some(before);
    Ok(())
}

#[then(regex = r#"^the frame "([^"]+)" is back where it was$"#)]
fn frame_back(w: &mut UiWorld, target: String) -> StepResult {
    let before = w.frame_before.ok_or("no place noted")?;
    let now = frame_bounds(w.window()?, &target)?;
    if (now.y0 - before.y0).abs() > 1.0 || (now.x0 - before.x0).abs() > 1.0 {
        return Err(format!("the frame went from {before:?} to {now:?}"));
    }
    Ok(())
}

#[then(regex = r#"^the button "([^"]+)" is (on|off)$"#)]
fn button_state(w: &mut UiWorld, name: String, state: String) -> StepResult {
    let tree = w.window()?.tree();
    let button = find(&tree, &|n| n.name == name && n.role == "button")
        .ok_or_else(|| format!("no button {name:?}; the window holds {}", names(&tree)))?;
    let on = button.states.iter().any(|s| s == "checked");
    (on == (state == "on"))
        .then_some(())
        .ok_or_else(|| format!("{name:?} reads {:?}", button.states))
}

#[then("the colour picker goes on from grey, with no reading in its hex")]
fn picker_from_grey(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    let tree = window.tree();
    let hex = find(&tree, &|n| n.name == "Colour" && n.key.is_some())
        .ok_or_else(|| format!("no colour field; the window holds {}", names(&tree)))?;
    if !hex.value.is_empty() {
        return Err(format!("the hex holds {:?}", hex.value));
    }
    let slider = find(&tree, &|n| n.role == "slider" && n.name == "Hue")
        .ok_or_else(|| format!("no sliders; the window holds {}", names(&tree)))?;
    if slider.states.iter().any(|s| s == "disabled") {
        return Err("the sliders are off".into());
    }
    // A slider's value is not in the accessibility tree.
    let value_of = |name: &str| -> Result<f64, String> {
        let slider = ljos_drive::find_below(window.engine(), window.root, name)
            .ok_or_else(|| format!("no slider {name:?}"))?;
        let read = window.engine().read_by_name(slider, "value");
        <f64 as QmlValue>::from_value(&read).map_err(|_| format!("{name:?} reads {read:?}"))
    };
    let brightness = value_of("pickerBrightness")?;
    if (brightness - 0.5).abs() > 0.01 {
        return Err(format!("brightness is {brightness}, not grey's"));
    }
    let saturation = value_of("pickerSaturation")?;
    if saturation.abs() > 0.01 {
        return Err(format!("saturation is {saturation}, not grey's"));
    }
    Ok(())
}

#[then("the inspector asks for something to be selected")]
fn inspector_asks(w: &mut UiWorld) -> StepResult {
    let window = w.window()?;
    window.open_panel("Inspector")?;
    let tree = window.tree();
    find(&tree, &|n| n.name == "Select something to edit it")
        .map(drop)
        .ok_or_else(|| {
            format!(
                "the inspector does not ask; the window holds {}",
                names(&tree)
            )
        })
}
