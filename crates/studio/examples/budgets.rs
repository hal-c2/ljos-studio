//! Studio's window budgets (DESIGN.md section 6), measured in wall time on
//! the shell Studio ships, in-process on a window without a platform:
//!
//! - cold start: a 200-file project opened, the shell made and its first
//!   frame shown with the documents listed;
//! - selection → inspector: a click on a layer until the inspector shows
//!   that object's bindings;
//! - inspector edit → picture: Enter in the inspector until the frame draws
//!   the preview's next generation, back to back and at a person's pace
//!   (needs `LJOS`, the `ljos` program the preview runs).
//!
//! The window's clock is a test clock, so a frame costs only its work; the
//! tool calls, the preview's reload and the picture's fetch and decode run
//! for real. Run it in a release build:
//!
//! ```text
//! LJOS=…/ljos cargo run --release -p studio --example budgets [-- ROUNDS]
//! ```

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow, bail};
use ljos::item::MouseButtons;
use ljos::prelude::*;
use ljos::testing::AccessibleNode;
use ljos_drive::{Chord, Stage};
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
/// The value the root's state changes, as the inspector names it: an object
/// the selection rounds leave alone, so a state that is always on does not
/// hide their edits of `width`.
const STATE_FIELD: &str = "card7.width";

fn main() -> anyhow::Result<()> {
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
    let mut edits = window.edits("width", rounds, 100, None)?;
    report("inspector edit -> new picture, back to back", &mut edits);
    // The back-to-back calls leave the inspector's one-second window full;
    // the paced edits start once it has emptied.
    std::thread::sleep(Duration::from_secs(1));
    let mut edits = window.edits("width", rounds, 200, Some(PACE))?;
    report(
        &format!(
            "inspector edit -> new picture, one per {} ms",
            PACE.as_millis()
        ),
        &mut edits,
    );

    std::thread::sleep(Duration::from_secs(1));
    let row = window.layer("Rectangle root")?;
    window.click(&row)?;
    let state = window.node_with("Inspector", "State on", |n| n.role == "button")?;
    window.click(&state)?;
    let mut edits = window.edits(STATE_FIELD, rounds, 20, Some(PACE))?;
    report(
        &format!(
            "state edit in the inspector -> new picture, one per {} ms",
            PACE.as_millis()
        ),
        &mut edits,
    );
    Ok(())
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

struct Window {
    stage: Stage,
    root: ObjectId,
    workspace: ObjectId,
}

impl Window {
    /// Opens Studio on `project` until its first frame lists the documents.
    fn open(project: &Path, state: &Path) -> anyhow::Result<Window> {
        let state = state.to_owned();
        let studio = Studio::open(project)?.with_previews(Arc::new(move |dir| {
            Box::new(NativeHost::new(dir, NativeHost::default_program(), &state))
        }));
        let server = Arc::new(StudioServer::new(studio));
        let engine = ljos::engine();
        studio_blocks::register(&engine);
        let mut stage = Stage::new(&engine, WIDTH, HEIGHT);
        studio_blocks::serve(&engine, server);
        let source = ljos::include_qml!("shell.pack");
        let root = ljos::create_root(&engine, source, "Main")?;
        stage.scene.set_root(&engine, root)?;
        stage.scene.set_accessibility_active(&engine, true);
        let workspace = find_workspace(&engine, root).ok_or(anyhow!("no Studio.Workspace"))?;
        let mut window = Window {
            stage,
            root,
            workspace,
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
    ) -> anyhow::Result<Vec<Duration>> {
        let mut edits = Vec::new();
        let mut last: Option<Instant> = None;
        for round in 0..rounds {
            if let (Some(pace), Some(last)) = (pace, last) {
                std::thread::sleep(pace.saturating_sub(last.elapsed()));
            }
            let shown = self.picture()?;
            let field = self.inspector_field(field)?;
            let started = Instant::now();
            last = Some(started);
            self.enter(&field, &format!("{}", first + round))?;
            self.wait_for_picture(shown + 1)?;
            edits.push(started.elapsed());
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
    fn wait_for_picture(&mut self, generation: u64) -> anyhow::Result<()> {
        let deadline = Instant::now() + CATCH_UP;
        loop {
            self.rest()?;
            let shown = self.picture()?;
            if shown >= generation {
                return Ok(());
            }
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
