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
use studio_core::preview::{
    Declared, LogLine, Picture, PreviewError, PreviewHost, PreviewSpec, Reload, Size,
};

use inspector::{Answer, Fault, Inspector};
pub use state_dir::private_dir;

/// How long a preview may take to answer its first call: a cold start
/// compiles the whole project.
const START_LIMIT: Duration = Duration::from_secs(30);

/// The capabilities the child grants Studio: reading, resizing (`input`),
/// assigning mocked state and reloading (`mutate`). Never `eval`.
const CAPS: &str = "inspect,input,mutate";

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
            .arg("--size")
            .arg(format!("{}x{}", spec.size.width, spec.size.height));
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
            PreviewError::Start(format!("cannot run `{}`: {e}", self.ljos.display()))
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
        })
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

impl PreviewHost for NativeHost {
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
        reload(&answer.structured)
    }

    fn alias(&mut self, module: &str, dir: Option<&str>) -> Result<Reload, PreviewError> {
        let dir = dir.map(|d| self.root.join(d).display().to_string());
        let answer = self.call("app_alias", json!({ "module": module, "dir": dir }))?;
        reload(&answer.structured)
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
        Ok(answer.text.unwrap_or_else(|| answer.structured.to_string()))
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
        .map_err(|e| no_object(e, id))?;
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

    fn close(&mut self) {
        if let Some(mut running) = self.running.take() {
            let _ = running.child.stop();
        }
    }
}

impl NativeHost {
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
