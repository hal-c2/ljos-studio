//! The native preview host: the document under edit runs in a child
//! `ljos run --headless --watch --mcp` process, and Studio drives it through
//! that application's inspector (Ljós ARCHITECTURE.md section 19.4), the
//! same tools an agent attached to any Ljós app has.
//!
//! The child watches the project as `ljos run --watch` always does, so a
//! file changed outside Studio shows too; after its own writes Studio asks
//! for the reload at once (`app_reload`) instead of waiting out the
//! watcher's quiet time, which is most of the edit-to-preview budget.

mod inspector;
mod state_dir;

use std::{
    fs::File,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use ljos_child::Reaped;
use serde_json::{Map, Value, json};
use studio_core::place::Rect;
use studio_core::preview::{
    Click, Declared, LogLine, Picture, Placed, PreviewError, PreviewHost, PreviewSpec, Reload,
    Rest, RestWatch, Size,
};
use studio_core::scene3d::{Body, Camera, Scene, rotate};

use inspector::{Answer, Fault, Inspector};
pub use state_dir::private_dir;

/// How long a preview may take to answer its first call: a cold start
/// compiles the whole project.
const START_LIMIT: Duration = Duration::from_secs(30);

/// The capabilities the child grants Studio: reading, resizing (`input`),
/// assigning mocked state and reloading (`mutate`). Never `eval`.
const CAPS: &str = "inspect,input,mutate";
/// The calls a second the child's inspector grants Studio, its only client:
/// a person's gestures and an agent's edits through Studio each ask for a
/// reload and a picture, and Ljós's default of 20 kept them waiting on its
/// one-second `Retry-After` at a brisk pace.
const CALLS_PER_SECOND: u32 = 200;

/// Objects read to find those without an id, in one call: past this a call
/// would spend the inspector's whole second, and the object is reported as
/// not found rather than making the person wait.
const MOST_READ: usize = 150;
/// Copies of a delegate read for their places.
const MOST_COPIES: usize = 16;

/// The project's type dumps run as one, in the state directory.
const MERGED_TYPES: &str = "types.json";

/// A preview as a child process.
pub struct NativeHost {
    /// The project's directory: documents and alias directories are below it.
    root: PathBuf,
    /// The `ljos` program.
    ljos: PathBuf,
    /// Where the child's token file and its stderr go, and where it runs:
    /// `ljos run --mcp` writes its token file below its working directory.
    state: PathBuf,
    running: Option<Running>,
}

struct Running {
    child: Reaped,
    inspector: Inspector,
    log: PathBuf,
    /// The last generation a reload was reported applied at.
    reported: u64,
}

impl std::fmt::Debug for NativeHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeHost")
            .field("root", &self.root)
            .field("ljos", &self.ljos)
            .field("running", &self.running.as_ref().map(|r| r.child.id()))
            .finish()
    }
}

impl NativeHost {
    /// A host for the project at `root` that runs `ljos` and keeps what the
    /// child writes below `state`.
    pub fn new(
        root: impl Into<PathBuf>,
        ljos: impl Into<PathBuf>,
        state: impl Into<PathBuf>,
    ) -> Self {
        // The child runs in `state`: a relative path would name a place
        // below it instead of below this process's working directory.
        let ljos = ljos.into();
        Self {
            root: absolute(root.into()),
            ljos: if ljos.components().count() > 1 {
                absolute(ljos)
            } else {
                // A bare name is looked up on the `PATH`.
                ljos
            },
            state: absolute(state.into()),
            running: None,
        }
    }

    /// The `ljos` program: `LJOS` when set, else `ljos` on the `PATH`.
    pub fn default_program() -> PathBuf {
        std::env::var_os("LJOS").map_or_else(|| PathBuf::from("ljos"), PathBuf::from)
    }

    fn reported(&mut self, answer: &Value) -> Result<Reload, PreviewError> {
        let running = self.running()?;
        since(answer, &mut running.reported)
    }

    fn running(&mut self) -> Result<&mut Running, PreviewError> {
        let running = self.running.as_mut().ok_or(PreviewError::NotOpen)?;
        if let Ok(Some(status)) = running.child.try_wait() {
            let message = format!("{status}; {}", tail(&running.log));
            self.running = None;
            return Err(PreviewError::Ended(message));
        }
        self.running.as_mut().ok_or(PreviewError::NotOpen)
    }

    fn call(&mut self, tool: &str, args: Value) -> Result<Answer, PreviewError> {
        let running = self.running()?;
        match running.inspector.call(tool, args) {
            Ok(answer) => Ok(answer),
            Err(Fault::Refused { code, message }) => Err(PreviewError::Refused { code, message }),
            Err(Fault::Transport(message)) => {
                // A refused connection is most often the child gone.
                let ended = matches!(running.child.try_wait(), Ok(Some(_)));
                if ended {
                    let message = format!("{message}; {}", tail(&running.log));
                    self.running = None;
                    return Err(PreviewError::Ended(message));
                }
                Err(PreviewError::Transport(message))
            }
        }
    }

    fn start(&self, spec: &PreviewSpec) -> Result<Running, PreviewError> {
        let entry = self.root.join(&spec.entry);
        if !entry.is_file() {
            return Err(PreviewError::Start(format!("no document `{}`", spec.entry)));
        }
        private_dir(&self.state)?;
        let log = self.state.join("preview.log");
        let stderr = File::create(&log)
            .map_err(|e| PreviewError::Start(format!("{}: {e}", log.display())))?;
        let mut command = Command::new(&self.ljos);
        command
            .arg("run")
            .arg(&entry)
            .arg("--project")
            .arg(&self.root)
            .arg("--headless")
            .arg("--watch")
            .arg(format!("--mcp={CAPS}"))
            .arg("--mcp-rate")
            .arg(CALLS_PER_SECOND.to_string())
            .arg("--size")
            .arg(format!("{}x{}", spec.size.width, spec.size.height));
        match spec.types.as_slice() {
            [] => {}
            [one] => {
                command.arg("--types").arg(self.root.join(one));
            }
            several => {
                command.arg("--types").arg(self.merged(several)?);
            }
        }
        for alias in &spec.aliases {
            command.arg("--alias").arg(format!(
                "{}={}",
                alias.module,
                self.root.join(&alias.dir).display()
            ));
        }
        command
            .current_dir(&self.state)
            .env("LJOS_MCP_DIR", &self.state)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr);
        let mut child = Reaped::spawn(&mut command).map_err(|e| {
            // The first thing a new checkout meets: say the way out.
            let hint = if e.kind() == std::io::ErrorKind::NotFound {
                "; `mise run setup` installs the pinned one, or name one with LJOS=/path/to/ljos"
            } else {
                ""
            };
            PreviewError::Start(format!("cannot run `{}`: {e}{hint}", self.ljos.display()))
        })?;
        let deadline = Instant::now() + START_LIMIT;
        let inspector = Inspector::connect(&self.state, child.id(), deadline, || {
            matches!(child.try_wait(), Ok(Some(_)))
        })
        .map_err(|e| PreviewError::Start(format!("{e}; {}", tail(&log))))?;
        Ok(Running {
            child,
            inspector,
            log,
            reported: 0,
        })
    }
}

impl NativeHost {
    /// The project's dumps as one, in the state directory: `ljos run` reads
    /// one file, and a project may keep a dump beside each crate that
    /// registers types. Their modules and unnamed types are put side by
    /// side, as a build that links every crate has them all.
    fn merged(&self, files: &[String]) -> Result<PathBuf, PreviewError> {
        let mut format = None;
        let (mut modules, mut unnamed) = (Vec::new(), Vec::new());
        for file in files {
            let fail = |why: String| PreviewError::Start(format!("{file}: {why}"));
            let text =
                std::fs::read_to_string(self.root.join(file)).map_err(|e| fail(e.to_string()))?;
            let dump: Value = serde_json::from_str(&text).map_err(|e| fail(e.to_string()))?;
            let this = dump.get("format").cloned().unwrap_or(Value::Null);
            match &format {
                None => format = Some(this),
                Some(first) if *first != this => {
                    return Err(fail(format!(
                        "its format is {this}, and {}'s is {first}",
                        files[0]
                    )));
                }
                Some(_) => {}
            }
            for (key, into) in [("modules", &mut modules), ("unnamed", &mut unnamed)] {
                match dump.get(key) {
                    None => {}
                    Some(Value::Array(list)) => into.extend(list.iter().cloned()),
                    Some(_) => return Err(fail(format!("`{key}` is not a list"))),
                }
            }
        }
        let path = self.state.join(MERGED_TYPES);
        let merged = json!({
            "format": format,
            "modules": modules,
            "unnamed": unnamed,
        });
        std::fs::write(&path, merged.to_string())
            .map_err(|e| PreviewError::Start(format!("{}: {e}", path.display())))?;
        Ok(path)
    }
}

/// `path` from the root, or as given when there is no working directory to
/// resolve it in, which the child's start then reports.
fn absolute(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        self.close();
    }
}

/// `app_watch` on the inspector's own thread: the app's UI thread is not woken
/// by it, so a preview at rest costs one sleeping call. Not `paints`, which a
/// preview without a window never counts, nor `frames`, which a screenshot
/// moves: what the app says when it sleeps is true of both and moved by neither.
struct Rests(Inspector);

impl RestWatch for Rests {
    fn wait(&self, since: Option<u64>, wait_ms: u64) -> Result<Rest, PreviewError> {
        let mut args = json!({ "topics": ["rests"], "wait_ms": wait_ms });
        if let Some(since) = since {
            args["since"] = json!({ "rests": since });
        }
        rest_of(self.0.call("app_watch", args))
    }

    fn parked(
        &self,
        since: u64,
        wait_ms: u64,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Rest, PreviewError>> + Send>> {
        let args = json!({
            "topics": ["rests"],
            "wait_ms": wait_ms,
            "since": { "rests": since },
        });
        let asking = self.0.asking("app_watch", args);
        Box::pin(async move { rest_of(asking.await) })
    }
}

fn rest_of(answer: Result<Answer, Fault>) -> Result<Rest, PreviewError> {
    match answer {
        Ok(answer) => {
            let told = &answer.structured;
            Ok(Rest {
                rests: told["counts"]["rests"]
                    .as_u64()
                    .ok_or_else(|| PreviewError::Transport("app_watch gave no rests".to_owned()))?,
                wake_in_ms: told["rest"]["wake_in_ms"].as_u64(),
            })
        }
        Err(Fault::Refused { code, message }) => Err(PreviewError::Refused { code, message }),
        Err(Fault::Transport(message)) => Err(PreviewError::Transport(message)),
    }
}

impl PreviewHost for NativeHost {
    fn calls_per_second(&self) -> u32 {
        CALLS_PER_SECOND
    }

    fn rest_watch(&self) -> Option<Box<dyn RestWatch>> {
        let running = self.running.as_ref()?;
        Some(Box::new(Rests(running.inspector.clone())))
    }

    fn open(&mut self, spec: &PreviewSpec) -> Result<(), PreviewError> {
        self.close();
        let running = self.start(spec)?;
        self.running = Some(running);
        // The first answer after the root is made: the preview shows it.
        self.call("app_status", json!({}))?;
        Ok(())
    }

    fn reload(&mut self) -> Result<Reload, PreviewError> {
        let answer = self.call("app_reload", json!({}))?;
        self.reported(&answer.structured)
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        let dir = dir.map(|d| self.root.join(d).display().to_string());
        let answer = self.call("app_alias", json!({ "module": module, "dir": dir }))?;
        self.reported(&answer.structured)
    }

    fn resize(&mut self, size: Size) -> Result<(), PreviewError> {
        self.call(
            "app_resize",
            json!({ "width": size.width, "height": size.height }),
        )?;
        Ok(())
    }

    fn screenshot(&mut self, max_side: u32) -> Result<Picture, PreviewError> {
        let answer = self.call("app_screenshot", json!({ "max_size": max_side }))?;
        let side = |name: &str| {
            answer.structured[name]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0)
        };
        let (width, height) = (side("width"), side("height"));
        let png = answer
            .images
            .into_iter()
            .find(|(mime, _)| mime == "image/png")
            .map(|(_, data)| data)
            .ok_or_else(|| PreviewError::Transport("the screenshot had no PNG".to_owned()))?;
        Ok(Picture { png, width, height })
    }

    fn outline(&mut self) -> Result<String, PreviewError> {
        let answer = self.call("app_snapshot", json!({}))?;
        let shown = answer.text.unwrap_or_else(|| answer.structured.to_string());
        let listed = self.call("app_objects", json!({ "max": 5000 }))?;
        let rows = listed.structured["objects"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let items = self.item_lines(&rows)?;
        Ok(if items.is_empty() {
            shown
        } else {
            format!("{shown}\n\nItems:\n{items}")
        })
    }

    fn read(&mut self, id: &str, names: &[String]) -> Result<Map<String, Value>, PreviewError> {
        let answer = self
            .call("app_inspect", json!({ "target": locator(id) }))
            .map_err(|e| no_object(e, id))?;
        let mut out = Map::new();
        let rows = answer.structured["properties"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for name in names {
            let row = rows.iter().find(|r| r["name"] == name.as_str());
            out.insert(name.clone(), row.map_or(Value::Null, value_of));
        }
        Ok(out)
    }

    fn write(&mut self, id: &str, name: &str, value: Value) -> Result<(), PreviewError> {
        self.call(
            "app_set_property",
            json!({ "target": locator(id), "property": name, "value": value }),
        )
        .map_err(|e| no_object(e, id))
        .map_err(|e| match e {
            // The inspector says the same of both, and only one of them means
            // the object is not there.
            PreviewError::NoObject(_) if self.read(id, &[]).is_ok() => PreviewError::Refused {
                code: "no_property".to_owned(),
                message: format!("`{id}` has no property `{name}`"),
            },
            e => e,
        })?;
        Ok(())
    }

    fn logs(&mut self, since: u64) -> Result<(Vec<LogLine>, u64), PreviewError> {
        let answer = self.call("app_logs", json!({ "since": since }))?;
        let lines = answer.structured["entries"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| LogLine {
                        level: e["level"].as_str().unwrap_or("info").to_owned(),
                        message: e["message"].as_str().unwrap_or_default().to_owned(),
                        file: e["file"].as_str().map(str::to_owned),
                        line: e["line"].as_u64().and_then(|l| u32::try_from(l).ok()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let next = answer.structured["next_since"].as_u64().unwrap_or(since);
        Ok((lines, next))
    }

    fn alive(&mut self) -> Result<(), PreviewError> {
        self.running().map(|_| ())
    }

    fn click(&mut self, at: &Click) -> Result<(), PreviewError> {
        match at {
            Click::Id(id) => self
                .call("app_click", json!({ "target": locator(id) }))
                .map_err(|e| no_object(e, id))?,
            // At the point itself, through the hit test: whatever is on top
            // there gets it, as under a person's pointer, not the middle of
            // the topmost item drawn there.
            Click::Point { x, y } => self.call("app_click", json!({ "at": [x, y] }))?,
        };
        Ok(())
    }

    fn pick(&mut self, x: f64, y: f64) -> Result<Option<Declared>, PreviewError> {
        let answer = match self.call("app_inspect", json!({ "target": format!("at={x},{y}") })) {
            Ok(answer) => answer,
            Err(PreviewError::Refused { code, .. })
                if code == "not_found" || code == "no_match" =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        let source = &answer.structured["source"];
        let Some(file) = source["file"].as_str() else {
            return Ok(None);
        };
        let number = |name: &str| {
            source[name]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        };
        Ok(Some(Declared {
            file: self.document(file),
            line: number("line"),
            column: number("column"),
        }))
    }

    fn scene(&mut self, at: &Declared) -> Result<Option<Scene>, PreviewError> {
        let listed = self.call("app_objects", json!({ "max": 5000 }))?;
        let rows = listed.structured["objects"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let declared: Vec<Option<Declared>> = rows
            .iter()
            .map(|r| r["at"].as_str().and_then(|a| self.declared_of(a)))
            .collect();
        let depth = |i: usize| rows[i]["depth"].as_u64().unwrap_or(0);
        let ty = |i: usize| rows[i]["type"].as_str().unwrap_or_default();
        let Some(here) = declared.iter().position(|d| d.as_ref() == Some(at)) else {
            return Ok(None);
        };
        // The nearest view around the object, or the object itself.
        let mut view = (ty(here) == "View3D").then_some(here);
        let mut below = depth(here);
        for i in (0..here).rev() {
            if view.is_some() {
                break;
            }
            if depth(i) < below {
                below = depth(i);
                if ty(i) == "View3D" {
                    view = Some(i);
                }
            }
        }
        let Some(view) = view else {
            return Ok(None);
        };
        let end = (view + 1..rows.len())
            .find(|i| depth(*i) <= depth(view))
            .unwrap_or(rows.len());
        let inner = view + 1..end;
        let inspected = |host: &mut Self, i: usize| -> Result<Value, PreviewError> {
            let handle = rows[i]["handle"].as_str().unwrap_or_default();
            Ok(host
                .call("app_inspect", json!({ "target": handle, "all": true }))?
                .structured)
        };
        let seen = inspected(self, view)?;
        let window = seen["geometry"]["window"]
            .as_array()
            .filter(|w| w.len() == 4)
            .map(|w| {
                w.iter()
                    .map(|n| n.as_f64().unwrap_or(0.0))
                    .collect::<Vec<_>>()
            });
        let Some(window) = window else {
            return Ok(None);
        };
        let view_box = Rect {
            x: window[0],
            y: window[1],
            width: window[2],
            height: window[3],
        };
        // The view's own `camera`, else the first the scene declares.
        let chosen = prop(&seen, "camera")
            .and_then(|r| r["handle"].as_str())
            .and_then(|h| inner.clone().find(|i| rows[*i]["handle"] == h))
            .or_else(|| inner.clone().find(|i| ty(*i).ends_with("Camera")));
        let camera = match chosen {
            Some(i) => {
                let c = inspected(self, i)?;
                let perspective = c["types"]
                    .as_array()
                    .is_some_and(|t| t.iter().any(|t| t == "PerspectiveCamera"));
                Some(Camera {
                    position: vec3(&c, "scenePosition").unwrap_or([0.0; 3]),
                    rotation: quat(&c, "sceneRotation").unwrap_or([1.0, 0.0, 0.0, 0.0]),
                    field_of_view: number_of(&c, "fieldOfView").unwrap_or(60.0),
                    perspective,
                })
            }
            None => None,
        };
        let mut bodies = Vec::new();
        for i in inner.filter(|i| ty(*i) == "Model").take(MOST_READ) {
            let Some(declared) = declared[i].clone() else {
                continue;
            };
            let m = inspected(self, i)?;
            // `visible: false` takes the model out of the picture and so out
            // of the click.
            if prop(&m, "visible").is_some_and(|r| r["value"] == "false") {
                continue;
            }
            let source = prop(&m, "source")
                .and_then(|r| r["value"].as_str())
                .unwrap_or_default()
                .to_owned();
            let parent = (0..i).rev().find(|p| depth(*p) + 1 == depth(i));
            let axes = match parent.filter(|p| *p != view) {
                Some(p) => {
                    let p = inspected(self, p)?;
                    let rotation = quat(&p, "sceneRotation").unwrap_or([1.0, 0.0, 0.0, 0.0]);
                    let scale = vec3(&p, "sceneScale").unwrap_or([1.0; 3]);
                    let along = |k: usize| {
                        let mut unit = [0.0; 3];
                        unit[k] = scale[k];
                        rotate(rotation, unit)
                    };
                    [along(0), along(1), along(2)]
                }
                None => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            };
            bodies.push(Body {
                declared,
                source,
                position: vec3(&m, "scenePosition").unwrap_or([0.0; 3]),
                rotation: quat(&m, "sceneRotation").unwrap_or([1.0, 0.0, 0.0, 0.0]),
                scale: vec3(&m, "sceneScale").unwrap_or([1.0; 3]),
                axes,
            });
        }
        Ok(Some(Scene {
            view: view_box,
            camera,
            bodies,
        }))
    }

    fn placed(&mut self, ids: &[String], types: &[String]) -> Result<Vec<Placed>, PreviewError> {
        let mut out = Vec::new();
        for id in ids {
            out.extend(self.placed_by_id(id)?);
        }
        if types.is_empty() {
            return Ok(out);
        }
        // A row has the box in the window but not the box in its parent, so
        // an object with no id takes reading objects of its type one by one.
        let listed = self.call("app_objects", json!({ "max": 5000 }))?;
        let mut read = 0;
        let rows = listed.structured["objects"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for row in rows {
            let of_type = row["type"]
                .as_str()
                .is_some_and(|ty| types.iter().any(|t| t == ty));
            if !of_type || row["id"].is_string() {
                continue;
            }
            if read == MOST_READ {
                break;
            }
            read += 1;
            let Some(handle) = row["handle"].as_str() else {
                continue;
            };
            let answer = self.call("app_inspect", json!({ "target": handle }))?;
            out.extend(self.placed_from(&answer.structured));
        }
        Ok(out)
    }

    fn close(&mut self) {
        if let Some(mut running) = self.running.take() {
            let _ = running.child.stop();
        }
    }
}

impl NativeHost {
    /// The object with the QML id `id`, or each copy when a delegate has it.
    fn placed_by_id(&mut self, id: &str) -> Result<Vec<Placed>, PreviewError> {
        match self.call("app_inspect", json!({ "target": locator(id) })) {
            Ok(answer) => Ok(self.placed_from(&answer.structured).into_iter().collect()),
            Err(PreviewError::Refused { code, .. })
                if code == "not_found" || code == "no_match" =>
            {
                Ok(Vec::new())
            }
            Err(PreviewError::Refused { code, .. }) if code == "ambiguous" => {
                let found = self.call("app_objects", json!({ "max": 5000 }))?;
                let rows = found.structured["objects"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let mut out = Vec::new();
                for row in rows.iter().filter(|r| r["id"] == id).take(MOST_COPIES) {
                    let Some(handle) = row["handle"].as_str() else {
                        continue;
                    };
                    let answer = self.call("app_inspect", json!({ "target": handle }))?;
                    out.extend(self.placed_from(&answer.structured));
                }
                Ok(out)
            }
            Err(e) => Err(e),
        }
    }

    /// The project's visible items, one line each, indented by depth. The
    /// snapshot above them only names what has an accessible role, so a
    /// card of a `Rectangle` and a `Text` shows here. What a style's or a
    /// module's file declares is left out: it is not the document's to read.
    fn item_lines(&mut self, rows: &[Value]) -> Result<String, PreviewError> {
        let mut lines = Vec::new();
        let mut read = 0;
        let mut hidden_below = None;
        for row in rows {
            let depth = row["depth"].as_u64().unwrap_or(0);
            if hidden_below.is_some_and(|hidden| depth > hidden) {
                continue;
            }
            hidden_below = None;
            if row["visible"] == false {
                hidden_below = Some(depth);
                continue;
            }
            let (Some(bounds), Some(at)) = (row["bounds"].as_array(), row["at"].as_str()) else {
                continue;
            };
            let at = at.rsplit_once(':').map_or(at, |(line, _column)| line);
            let Some((file, line)) = at.rsplit_once(':') else {
                continue;
            };
            let file = self.document(file);
            if file.contains("://") || file.starts_with('/') {
                continue;
            }
            let number = |i: usize| bounds.get(i).and_then(Value::as_f64).unwrap_or(0.0);
            let id = row["id"]
                .as_str()
                .map(|id| format!("#{id}"))
                .unwrap_or_default();
            // A row does not carry what a text says, and it is what tells
            // one card of a list from the next.
            let mut says = String::new();
            let ty = row["type"].as_str().unwrap_or("object");
            if matches!(ty, "Text" | "Label") && read < MOST_READ {
                read += 1;
                if let Some(handle) = row["handle"].as_str() {
                    let answer = self.call("app_inspect", json!({ "target": handle }))?;
                    let rows = answer.structured["properties"].as_array();
                    if let Some(text) = rows
                        .and_then(|rows| rows.iter().find(|r| r["name"] == "text"))
                        .map(value_of)
                        .and_then(|v| v.as_str().map(str::to_owned))
                    {
                        says = format!(" {text:?}");
                    }
                }
            }
            lines.push(format!(
                "{}{ty}{id} {},{} {}x{} {file}:{line}{says}",
                "  ".repeat(usize::try_from(depth).unwrap_or(0)),
                number(0),
                number(1),
                number(2),
                number(3),
            ));
        }
        Ok(lines.join("\n"))
    }

    /// An `app_inspect` answer as a [`Placed`]; `None` for what is not an item.
    fn placed_from(&self, answer: &Value) -> Option<Placed> {
        let geometry = &answer["geometry"];
        let number = |v: &Value| v.as_f64().unwrap_or(0.0);
        let window = geometry["window"].as_array()?;
        let source = &answer["source"];
        let line = |name: &str| {
            source[name]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        };
        Some(Placed {
            local: Rect {
                x: number(&geometry["x"]),
                y: number(&geometry["y"]),
                width: number(&geometry["width"]),
                height: number(&geometry["height"]),
            },
            window: Rect {
                x: number(window.first()?),
                y: number(window.get(1)?),
                width: number(window.get(2)?),
                height: number(window.get(3)?),
            },
            declared: source["file"].as_str().map(|file| Declared {
                file: self.document(file),
                line: line("line"),
                column: line("column"),
            }),
            types: answer["types"]
                .as_array()
                .map(|types| {
                    types
                        .iter()
                        .filter_map(|t| t.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Where an `app_objects` row says an object is declared: `file:line:column`.
    fn declared_of(&self, at: &str) -> Option<Declared> {
        let (rest, column) = at.rsplit_once(':')?;
        let (file, line) = rest.rsplit_once(':')?;
        Some(Declared {
            file: self.document(file),
            line: line.parse().ok()?,
            column: column.parse().ok()?,
        })
    }

    /// A file the child names (a `file:` URL, as its engine does) as the
    /// store names it: relative to the project when it is below it.
    fn document(&self, file: &str) -> String {
        let Some(path) = file.strip_prefix("file://") else {
            return file.to_owned();
        };
        let path = PathBuf::from(percent_decoded(path));
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let path = path.canonicalize().unwrap_or(path);
        match path.strip_prefix(&root) {
            Ok(relative) => relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
            Err(_) => file.to_owned(),
        }
    }
}

/// A URL path with its `%XX` escapes turned back into bytes.
fn percent_decoded(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push(u8::try_from(hi * 16 + lo).unwrap_or(b'%'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The row of `answer` for the property `name`.
fn prop<'a>(answer: &'a Value, name: &str) -> Option<&'a Value> {
    answer["properties"]
        .as_array()?
        .iter()
        .find(|r| r["name"] == name)
}

/// The numbers in a value the inspector shows as text: `QVector3D(1, 2, 3)`.
fn numbers_in(text: &str) -> Vec<f64> {
    let inner = text.split_once('(').map_or(text, |(_, rest)| rest);
    inner
        .trim_end_matches(')')
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

fn vec3(answer: &Value, name: &str) -> Option<[f64; 3]> {
    numbers_in(prop(answer, name)?["value"].as_str()?)
        .try_into()
        .ok()
}

fn quat(answer: &Value, name: &str) -> Option<[f64; 4]> {
    numbers_in(prop(answer, name)?["value"].as_str()?)
        .try_into()
        .ok()
}

fn number_of(answer: &Value, name: &str) -> Option<f64> {
    let row = prop(answer, name)?;
    row["number"]
        .as_f64()
        .or_else(|| row["value"].as_str()?.parse().ok())
}

/// The inspector's locator for the object with the QML `id`.
fn locator(id: &str) -> String {
    format!("id={id}")
}

fn no_object(e: PreviewError, id: &str) -> PreviewError {
    match e {
        PreviewError::Refused { code, .. } if code == "not_found" || code == "no_match" => {
            PreviewError::NoObject(id.to_owned())
        }
        e => e,
    }
}

/// A property row of `app_inspect` as JSON. The inspector shows values as
/// text for people; the row's declared type says how to read it back.
fn value_of(row: &Value) -> Value {
    if let Some(n) = row.get("number").filter(|n| n.is_number()) {
        return n.clone();
    }
    let text = row["value"].as_str();
    match (row["type"].as_str(), text) {
        (Some("bool"), Some("true")) => Value::Bool(true),
        (Some("bool"), Some("false")) => Value::Bool(false),
        (_, Some(text)) => Value::String(text.to_owned()),
        _ => row["value"].clone(),
    }
}

/// What a reload's answer says the preview did since the one `reported`
/// last told of. The preview watches the files too, and its watcher may
/// apply a save before the reload asked for it arrives; that reload then
/// answers `unchanged` at a generation nobody was told of, which was
/// applied all the same.
fn since(answer: &Value, reported: &mut u64) -> Result<Reload, PreviewError> {
    let generation = answer["generation"].as_u64().unwrap_or(0);
    let outcome = match reload(answer)? {
        Reload::Unchanged if generation > *reported => Reload::Applied { generation },
        outcome => outcome,
    };
    if let Reload::Applied { generation } = outcome {
        *reported = generation;
    }
    Ok(outcome)
}

fn reload(answer: &Value) -> Result<Reload, PreviewError> {
    let generation = answer["generation"].as_u64().unwrap_or(0);
    match answer["outcome"].as_str() {
        Some("applied") => Ok(Reload::Applied { generation }),
        Some("failed") => Ok(Reload::Failed {
            message: answer["message"].as_str().unwrap_or_default().to_owned(),
        }),
        Some("unchanged") => Ok(Reload::Unchanged),
        _ => Err(PreviewError::Transport(format!(
            "a reload answered {answer}"
        ))),
    }
}

/// The last lines the child wrote to stderr, which say why it ended.
fn tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().rev().take(8).collect();
    if lines.is_empty() {
        return "it wrote nothing".to_owned();
    }
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_the_watcher_applied_first_is_reported_applied() {
        let mut reported = 3;
        let answer = json!({ "outcome": "unchanged", "generation": 4 });
        assert_eq!(
            since(&answer, &mut reported).unwrap(),
            Reload::Applied { generation: 4 }
        );
        assert_eq!(reported, 4);
        // Told of once: asked again, nothing changed.
        assert_eq!(since(&answer, &mut reported).unwrap(), Reload::Unchanged);
    }

    #[test]
    fn an_applied_reload_is_what_is_reported_next() {
        let mut reported = 0;
        let applied = json!({ "outcome": "applied", "generation": 1 });
        assert_eq!(
            since(&applied, &mut reported).unwrap(),
            Reload::Applied { generation: 1 }
        );
        let unchanged = json!({ "outcome": "unchanged", "generation": 1 });
        assert_eq!(since(&unchanged, &mut reported).unwrap(), Reload::Unchanged);
    }
}
