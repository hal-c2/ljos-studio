//! Studio's window scenarios (`features/ui/`): a person drives Studio's own
//! shell (`shell/`), with Studio's blocks registered, in a window without a
//! platform on a test clock. Steps find controls as an assistive technology
//! does, by role and name, and click, type and press keys as a person does;
//! the oracle is the file text, as for the MCP scenarios.

use std::sync::Arc;

use cucumber::{World, gherkin::Step, given, then, when};
use ljos::item::{DrawOp, MouseButtons, kurbo::Point};
use ljos::prelude::*;
use ljos::testing::AccessibleNode;
use ljos_drive::{Chord, DriveError, Stage};
use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::Value as Json;
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

/// The window's size: a laptop's screen, which a desktop frame at full size
/// does not fit beside the panels; the canvas scrolls.
const WIDTH: f64 = 1440.0;
const HEIGHT: f64 = 900.0;

#[derive(World)]
#[world(init = Self::new)]
struct UiWorld {
    dir: tempfile::TempDir,
    /// Where previews keep their token files and logs, apart from the
    /// project the preview watches.
    state: tempfile::TempDir,
    window: Option<Window>,
}

struct Window {
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
            dir: tempfile::tempdir().expect("a temporary directory"),
            state: tempfile::tempdir().expect("a temporary directory"),
            window: None,
        }
    }

    fn window(&mut self) -> Result<&mut Window, String> {
        self.window
            .as_mut()
            .ok_or_else(|| "Studio's window is not open".to_owned())
    }

    fn open(&mut self) -> StepResult {
        // A directory for the host to make, as the program names one: a
        // temporary directory itself is as open as the umask leaves it.
        let state = self.state.path().join("preview");
        let studio = Studio::open(self.dir.path())
            .map_err(|e| e.to_string())?
            .with_previews(Arc::new(move |dir| {
                Box::new(NativeHost::new(dir, NativeHost::default_program(), &state))
            }));
        let server = Arc::new(StudioServer::new(studio));
        let engine = ljos::engine();
        studio_blocks::register(&engine);
        let mut stage = Stage::new(&engine, WIDTH, HEIGHT);
        studio_blocks::serve(&engine, Arc::clone(&server));
        let source = ljos::dev::compile_dir_with(shell().as_ref(), &engine)
            .map_err(|e| format!("the shell does not compile: {e}"))?;
        if let Some((key, why)) = source.failures().next() {
            return Err(format!("the shell's {key} does not compile: {why}"));
        }
        let root = ljos::create_root(&engine, source, "Main")
            .map_err(|e| format!("the shell is not made: {e}"))?;
        stage
            .scene
            .set_root(&engine, root)
            .map_err(|e| e.to_string())?;
        stage.scene.set_accessibility_active(&engine, true);
        let workspace =
            find_workspace(&engine, root).ok_or("the shell binds no Studio.Workspace")?;
        let mut window = Window {
            stage,
            root,
            workspace,
            server,
            idle: None,
        };
        window.rest()?;
        self.window = Some(window);
        Ok(())
    }
}

fn shell() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../shell").to_owned()
}

/// The first `Studio.Workspace` under `root`, depth first.
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

impl Window {
    fn engine(&self) -> &Engine {
        &self.stage.engine
    }

    /// Lets everything the last gesture started finish: its tool calls,
    /// which run off the UI thread, what they publish, and the pictures
    /// they fetch, then the frames that show it.
    fn rest(&mut self) -> StepResult {
        let mut driver = self.stage.drive();
        driver.settle().map_err(drive_error)?;
        driver.finish_promised().map_err(drive_error)
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

    /// The node named `name` inside the one named `within`.
    fn node(&self, within: &str, role: Option<&str>, name: &str) -> Result<AccessibleNode, String> {
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
    fn shown_layers(&self) -> Result<Vec<AccessibleNode>, String> {
        let tree = self.tree();
        let list = find(&tree, &|n| n.name == "Layers" && n.role == "list")
            .ok_or("no layers list in the window")?;
        let mut rows = Vec::new();
        collect(list, &mut |n| {
            if n.key.is_some() && n.name != "Layers" && n.role != "label" && n.role != "statictext"
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
    std::fs::write(path, file_text(step)?).map_err(|e| e.to_string())
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
    window.click_node(&row)?;
    (window.read("doc").to_js_string().as_ref() as &str == doc)
        .then_some(())
        .ok_or_else(|| format!("the window shows {:?}", window.read("doc").to_js_string()))
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

#[when(regex = r#"^the person places "([^"]+)" from the palette$"#)]
fn places(w: &mut UiWorld, name: String) -> StepResult {
    let window = w.window()?;
    let button = window.node("Palette", None, &format!("Place {name}"))?;
    window.click_node(&button)
}

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

#[when(regex = r#"^the person presses "([^"]+)"$"#)]
fn presses(w: &mut UiWorld, chord: String) -> StepResult {
    let window = w.window()?;
    let chord = Chord::parse(&chord)?;
    window.stage.drive().chord(&chord, 0).map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^the person replaces "(.*)" with "(.*)" in the code view$"#)]
fn replaces_in_code(w: &mut UiWorld, old: String, new: String) -> StepResult {
    let (old, new) = (unescape(&old), unescape(&new));
    let window = w.window()?;
    let code = window.code()?;
    window.click_node(&code)?;
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
    window.stage.drive().commit(&new).map_err(drive_error)?;
    window.rest()
}

#[when(regex = r#"^an agent calls the tool "([^"]+)" with:$"#)]
fn agent_calls(w: &mut UiWorld, tool: String, step: &Step) -> StepResult {
    let args: Json = serde_json::from_str(&docstring(step)?).map_err(|e| e.to_string())?;
    let window = w.window()?;
    window
        .server
        .call(
            &tool,
            &CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate])),
            args,
        )
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

#[when(regex = r#"^nobody touches the window for (\d+) seconds?$"#)]
fn nobody_touches(w: &mut UiWorld, seconds: u64) -> StepResult {
    let window = w.window()?;
    window.rest()?;
    let before = window.stage.state.clone();
    window
        .stage
        .drive()
        .advance(seconds * 1000)
        .map_err(drive_error)?;
    let after = &window.stage.state;
    window.idle = Some(Idle {
        wake_before: before.wake,
        wake_after: after.wake,
        shown: after.shown - before.shown,
    });
    Ok(())
}

// ------------------------------------------------------------------- then

#[then(regex = r#"^the layers panel lists "([^"]*)"$"#)]
fn layers_list(w: &mut UiWorld, want: String) -> StepResult {
    let window = w.window()?;
    let want = list(&want);
    let shown: Vec<String> = window.shown_layers()?.into_iter().map(|n| n.name).collect();
    (shown == want)
        .then_some(())
        .ok_or_else(|| format!("the layers panel lists {shown:?}"))
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

#[then(regex = r#"^the inspector shows "([^"]+)" as "(.*)"$"#)]
fn inspector_shows(w: &mut UiWorld, name: String, value: String) -> StepResult {
    let value = unescape(&value);
    let window = w.window()?;
    let field = window.node("Inspector", Some("textinput"), &name)?;
    (field.value == value)
        .then_some(())
        .ok_or_else(|| format!("the inspector shows {name} as {:?}", field.value))
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

#[then(regex = r#"^the frame "([^"]+)" shows a picture of the preview$"#)]
fn frame_shows_picture(w: &mut UiWorld, target: String) -> StepResult {
    picture_drawn(w.window()?, &target).map(drop)
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

fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features/ui");
    let input = std::env::var("STUDIO_FEATURES").unwrap_or_else(|_| features.to_owned());
    // The Engine lives on one thread: one scenario at a time, on this one.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(
        UiWorld::cucumber()
            .max_concurrent_scenarios(1)
            .fail_on_skipped()
            .filter_run_and_exit(input, |feature, _, scenario| {
                feature.tags.iter().chain(&scenario.tags).any(|t| t == "ui")
            }),
    );
}
