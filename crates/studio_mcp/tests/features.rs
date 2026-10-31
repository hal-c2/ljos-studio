//! Studio's scenarios (`features/`): an agent drives `studio --mcp`'s tools
//! on a project in a temporary directory, and the scenarios assert what the
//! tools answer and what the files say, byte for byte (DESIGN.md: scenarios
//! assert file text).
//!
//! The vocabulary is Ljós's own for its MCP servers (`the agent calls the
//! tool …`, `the answer's "/pointer" is …`), so a scenario reads the same in
//! both repositories.
//!
//! Every answer is also held to the `outputSchema` the tool advertises: a
//! client SDK that validates rejects an answer that breaks it.

mod site;

use std::{collections::HashMap, sync::Arc};

use cucumber::{
    World,
    gherkin::{Step, StepType},
    given, then, when,
};
use ljos_mcp_core::{
    CallCtx, Cap, CapSet, McpSurface, SurfaceError, ToolImage, glue::http::HttpServer,
};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer, endpoint};
use studio_preview::NativeHost;

#[path = "support/schema.rs"]
mod schema;

#[derive(World)]
#[world(init = Self::new)]
struct StudioWorld {
    dir: tempfile::TempDir,
    /// Directories links lead out of the project to.
    outside: Vec<tempfile::TempDir>,
    /// Where previews keep their token files and logs: apart from the
    /// project, which the preview watches.
    state: tempfile::TempDir,
    server: Option<StudioServer>,
    caps: CapSet,
    answer: Option<Result<Value, SurfaceError>>,
    /// The pictures of the last answer.
    images: Vec<ToolImage>,
    /// Values kept from answers, written `$name` in later arguments.
    kept: HashMap<String, Value>,
    /// The endpoint a running Studio serves agents on.
    endpoint: Option<HttpServer>,
    /// What it answered last: status and body.
    reply: Option<(u16, String)>,
    /// How many reloads Studio had reported when the preview was last read.
    reloads_read: usize,
    /// How long the last tool call took to answer.
    took: std::time::Duration,
    /// Why Studio could not serve agents, when it could not.
    not_served: Option<String>,
}

impl std::fmt::Debug for StudioWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StudioWorld")
            .field("dir", &self.dir.path())
            .field("answer", &self.answer)
            .finish_non_exhaustive()
    }
}

type StepResult = Result<(), String>;

impl StudioWorld {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("a temporary directory"),
            outside: Vec::new(),
            state: tempfile::tempdir().expect("a temporary directory"),
            server: None,
            caps: CapSet::of([Cap::Inspect, Cap::Mutate]),
            answer: None,
            images: Vec::new(),
            kept: HashMap::new(),
            endpoint: None,
            reply: None,
            reloads_read: 0,
            took: std::time::Duration::ZERO,
            not_served: None,
        }
    }

    /// Where the endpoint keeps its token: a directory for the server to
    /// make, as Studio's state directory is the user's alone and a temporary
    /// directory is as open as the umask leaves it.
    fn endpoint_state(&self) -> std::path::PathBuf {
        self.state.path().join("endpoint")
    }

    fn server(&mut self) -> Result<&StudioServer, String> {
        if self.server.is_none() {
            // Previews run the `ljos` that `LJOS` names (`mise run features`
            // builds it from the Ljós checkout beside this one).
            // A directory for the host to make, as the program names one: a
            // temporary directory itself is as open as the umask leaves it.
            let state = self.state.path().join("preview");
            let studio = Studio::open(self.dir.path())
                .map_err(|e| e.to_string())?
                .with_previews(Arc::new(move |dir| {
                    Box::new(NativeHost::new(dir, NativeHost::default_program(), &state))
                }));
            self.server = Some(StudioServer::new(studio));
        }
        Ok(self.server.as_ref().expect("just made"))
    }

    fn call(&mut self, tool: &str, args: Value) -> StepResult {
        let caps = self.caps;
        let server = self.server()?;
        let started = std::time::Instant::now();
        let out = server.call(tool, &CallCtx::new(caps), args);
        let took = started.elapsed();
        let (out, images) = match out {
            Ok(reply) => (Ok(reply.structured), reply.images),
            Err(e) => (Err(e), Vec::new()),
        };
        // A refusal is an error result, which the output schema does not cover.
        let output = server.tools().all().find(|d| d.name == tool);
        if let (Ok(answer), Some(output)) = (&out, output.and_then(|d| d.output_schema.as_deref()))
        {
            let broken = schema::validate(&Value::Object(output.clone()), answer);
            if !broken.is_empty() {
                let broken: Vec<String> = broken.iter().map(ToString::to_string).collect();
                return Err(format!(
                    "`{tool}` answered against its outputSchema: {}\nanswer: {answer}",
                    broken.join("; ")
                ));
            }
        }
        self.answer = Some(out);
        self.images = images;
        self.took = took;
        Ok(())
    }

    /// What the open preview shows, as text; leaves the last answer be.
    fn preview_outline(&mut self) -> Result<String, String> {
        let caps = self.caps;
        let reply = self
            .server()?
            .call(
                "preview_screenshot",
                &CallCtx::new(caps),
                json!({ "format": "outline" }),
            )
            .map_err(|e| format!("the preview's outline was refused: {}", e.to_value()))?;
        reply.structured["outline"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("no outline in {}", reply.structured))
    }

    /// Every event of the open preview, all pages; leaves the last answer be.
    fn preview_events(&mut self) -> Result<Vec<Value>, String> {
        let caps = self.caps;
        let server = self.server()?;
        let mut events = Vec::new();
        loop {
            let reply = server
                .call(
                    "preview_events",
                    &CallCtx::new(caps),
                    json!({ "since": events.len() }),
                )
                .map_err(|e| format!("the preview's events were refused: {}", e.to_value()))?;
            let page = reply.structured["events"]
                .as_array()
                .ok_or_else(|| format!("no events in {}", reply.structured))?;
            if page.is_empty() {
                return Ok(events);
            }
            events.extend(page.iter().cloned());
        }
    }

    /// The reloads Studio reported, and every event as one line each: what
    /// a step that read the preview says when it was not what it wanted.
    fn reloads(events: &[Value]) -> (usize, String) {
        let reloads = events.iter().filter(|e| is_reload(e)).count();
        let lines: Vec<String> = events.iter().map(Value::to_string).collect();
        (reloads, lines.join("\n"))
    }

    fn answer(&self) -> Result<&Value, String> {
        match &self.answer {
            None => Err("no tool was called".to_owned()),
            Some(Err(e)) => Err(format!("the tool refused: {}", e.to_value())),
            Some(Ok(v)) => Ok(v),
        }
    }

    fn refusal(&self) -> Result<Value, String> {
        match &self.answer {
            Some(Err(e)) => Ok(e.to_value()["error"].clone()),
            Some(Ok(v)) => Err(format!("the tool answered: {v}")),
            None => Err("no tool was called".to_owned()),
        }
    }

    /// Writes kept values in for `"$name"` (a whole JSON string).
    fn arguments(&self, text: &str) -> Result<Value, String> {
        let mut value: Value =
            serde_json::from_str(text).map_err(|e| format!("the arguments are not JSON: {e}"))?;
        self.substitute(&mut value)?;
        Ok(value)
    }

    fn substitute(&self, value: &mut Value) -> StepResult {
        match value {
            Value::String(s) if s.starts_with('$') => {
                *value = self
                    .kept
                    .get(&s[1..])
                    .cloned()
                    .ok_or_else(|| format!("nothing was kept as `{}`", &s[1..]))?;
            }
            Value::Array(items) => {
                for item in items {
                    self.substitute(item)?;
                }
            }
            Value::Object(map) => {
                for item in map.values_mut() {
                    self.substitute(item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn docstring(step: &Step) -> Result<String, String> {
    let text = step
        .docstring
        .as_deref()
        .ok_or_else(|| "this step needs a doc string".to_owned())?;
    // The doc string's content type (`"""qml`) arrives as its first line.
    let text = match text.split_once('\n') {
        Some((first, rest)) if matches!(first.trim(), "qml" | "json" | "text") => rest,
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

fn at<'v>(value: &'v Value, pointer: &str) -> Result<&'v Value, String> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("nothing at `{pointer}` in {value}"))
}

fn json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| format!("`{text}` is not JSON: {e}"))
}

#[given("an empty project")]
fn empty_project(_: &mut StudioWorld) {}

#[given(regex = r#"^the file "([^"]+)":$"#)]
#[when(regex = r#"^somebody writes the file "([^"]+)":$"#)]
fn the_file(w: &mut StudioWorld, name: String, step: &Step) -> StepResult {
    let path = w.dir.path().join(&name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, file_text(step)?).map_err(|e| e.to_string())
}

#[given(regex = r#"^the file "([^"]+)" holds bytes that are not UTF-8$"#)]
fn not_utf8(w: &mut StudioWorld, name: String) -> StepResult {
    std::fs::write(w.dir.path().join(&name), b"Item { \xff }\n").map_err(|e| e.to_string())
}

#[when(regex = r#"^the file "([^"]+)" is rewritten outside Studio:$"#)]
fn rewritten(w: &mut StudioWorld, name: String, step: &Step) -> StepResult {
    std::fs::write(w.dir.path().join(&name), file_text(step)?).map_err(|e| e.to_string())
}

#[when(regex = r#"^the file "([^"]+)" is deleted outside Studio$"#)]
fn deleted(w: &mut StudioWorld, name: String) -> StepResult {
    std::fs::remove_file(w.dir.path().join(&name)).map_err(|e| e.to_string())
}

#[given(regex = r#"^the link "([^"]+)" leads to "([^"]+)"$"#)]
fn link_inside(w: &mut StudioWorld, name: String, to: String) -> StepResult {
    std::os::unix::fs::symlink(w.dir.path().join(to), w.dir.path().join(name))
        .map_err(|e| e.to_string())
}

#[given(
    regex = r#"^the link "([^"]+)" leads to a directory outside the project holding "([^"]+)"$"#
)]
fn link_outside(w: &mut StudioWorld, name: String, file: String) -> StepResult {
    let outside = tempfile::tempdir().map_err(|e| e.to_string())?;
    std::fs::write(outside.path().join(file), "Item {}\n").map_err(|e| e.to_string())?;
    // The link may be below directories the project does not have yet.
    let at = w.dir.path().join(name);
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::os::unix::fs::symlink(outside.path(), at).map_err(|e| e.to_string())?;
    w.outside.push(outside);
    Ok(())
}

#[given(regex = r#"^the link "([^"]+)" leads to the file "([^"]+)" outside the project$"#)]
fn link_file_outside(w: &mut StudioWorld, name: String, file: String) -> StepResult {
    let outside = tempfile::tempdir().map_err(|e| e.to_string())?;
    let target = outside.path().join(file);
    std::fs::write(&target, "Item {}\n").map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(target, w.dir.path().join(name)).map_err(|e| e.to_string())?;
    w.outside.push(outside);
    Ok(())
}

#[given(regex = r#"^the named pipe "([^"]+)"$"#)]
fn named_pipe(w: &mut StudioWorld, name: String) -> StepResult {
    let at = w.dir.path().join(name);
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // A reader of it waits for a writer that never comes.
    let made = std::process::Command::new("mkfifo")
        .arg(&at)
        .status()
        .map_err(|e| format!("mkfifo: {e}"))?;
    made.success()
        .then_some(())
        .ok_or_else(|| format!("mkfifo {}: {made}", at.display()))
}

#[then("nothing was written outside the project")]
fn nothing_outside(w: &mut StudioWorld) -> StepResult {
    // Each place a link leads to holds the one file the scenario put there.
    for outside in &w.outside {
        let entries: Vec<_> = std::fs::read_dir(outside.path())
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .collect();
        let untouched = matches!(entries.as_slice(), [one]
            if std::fs::read_to_string(one).is_ok_and(|text| text == "Item {}\n"));
        if !untouched {
            return Err(format!("{entries:?}"));
        }
    }
    Ok(())
}

#[given(
    regex = r#"^"([^"]+)" links to a directory outside the project that holds the file "([^"]+)"$"#
)]
fn links_out(w: &mut StudioWorld, link: String, file: String) -> StepResult {
    link_outside(w, link, file)
}

#[given(regex = r#"^"([^"]+)" links to "([^"]+)" in the project$"#)]
fn links_in(w: &mut StudioWorld, link: String, target: String) -> StepResult {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(w.dir.path().join(&target), w.dir.path().join(&link))
        .map_err(|e| e.to_string());
    #[cfg(not(unix))]
    return Err(format!("`{link}`: links are made on unix only"));
}

/// `levels` directories below `under`, `0` to `levels - 1`, each holding one
/// file (unless "holding no files") and, but the last, two links `left` and
/// `right` to the next: a diamond at every level, so every way down through
/// the links is another copy of all below.
#[given(
    regex = r#"^a diamond of directory links (\d+) levels deep in "([^"]+)"( holding no files)?$"#
)]
fn diamond(w: &mut StudioWorld, levels: usize, under: String, empty: String) -> StepResult {
    let under = w.dir.path().join(under);
    for level in 0..levels {
        let dir = under.join(level.to_string());
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        if empty.is_empty() {
            std::fs::write(dir.join("level.txt"), format!("{level}\n"))
                .map_err(|e| e.to_string())?;
        }
        if level + 1 == levels {
            continue;
        }
        #[cfg(unix)]
        for side in ["left", "right"] {
            std::os::unix::fs::symlink(under.join((level + 1).to_string()), dir.join(side))
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        return Err("links are made on unix only".to_owned());
    }
    Ok(())
}

#[then(regex = r#"^it answered within (\d+) seconds?$"#)]
fn answered_within(w: &mut StudioWorld, seconds: u64) -> StepResult {
    let limit = std::time::Duration::from_secs(seconds);
    (w.took <= limit)
        .then_some(())
        .ok_or_else(|| format!("it took {:?}", w.took))
}

#[then(regex = r#"^the directory outside the project holds only "([^"]*)"$"#)]
fn outside_holds(w: &mut StudioWorld, names: String) -> StepResult {
    let outside = w.outside.last().ok_or("no directory outside the project")?;
    let mut held: Vec<String> = std::fs::read_dir(outside.path())
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    held.sort();
    let want: Vec<&str> = names.split(", ").filter(|n| !n.is_empty()).collect();
    if held != want {
        return Err(format!("it holds {held:?}, not {want:?}"));
    }
    for name in &held {
        let text = std::fs::read_to_string(outside.path().join(name)).map_err(|e| e.to_string())?;
        if text != "Item {}\n" {
            return Err(format!("`{name}` was written: {text:?}"));
        }
    }
    Ok(())
}

#[given("the studio tools are connected")]
fn connected(w: &mut StudioWorld) -> StepResult {
    w.server().map(|_| ())
}

#[given(regex = r#"^the studio tools are connected with the capabilities "([^"]*)"$"#)]
fn connected_with(w: &mut StudioWorld, caps: String) -> StepResult {
    w.caps = CapSet::parse_list(&caps).map_err(|e| e.to_string())?;
    w.server().map(|_| ())
}

#[given(regex = r#"^the agent has called the tool "([^"]+)"$"#)]
#[when(regex = r#"^the agent calls the tool "([^"]+)"$"#)]
fn calls(w: &mut StudioWorld, tool: String) -> StepResult {
    w.call(&tool, Value::Object(Default::default()))
}

#[given(regex = r#"^the agent has called the tool "([^"]+)" with:$"#)]
#[when(regex = r#"^the agent calls the tool "([^"]+)" with:$"#)]
fn calls_with(w: &mut StudioWorld, tool: String, step: &Step) -> StepResult {
    let args = w.arguments(&docstring(step)?)?;
    w.call(&tool, args)?;
    // A `Given` sets the scene (an `And` under one too): a refusal there is
    // the scenario's mistake.
    if step.ty == StepType::Given {
        w.answer()?;
    }
    Ok(())
}

#[when(regex = r#"^a person types in the code view of "([^"]+)", replacing "(.*)" with "(.*)"$"#)]
fn person_types(w: &mut StudioWorld, doc: String, old: String, new: String) -> StepResult {
    let server = w.server()?;
    let studio = server.studio();
    let mut studio = studio.lock().map_err(|e| e.to_string())?;
    let (text, generation) = studio.session.text(&doc).map_err(|e| e.to_string())?;
    // A step writes `\"` for a quote and `\n` for a line break.
    let unquote = |t: &str| t.replace("\\\"", "\"").replace("\\n", "\n");
    let old = unquote(&old);
    let start = text
        .find(&old)
        .ok_or_else(|| format!("`{old}` is not in {doc}"))?;
    let range = start..start + old.len();
    studio
        .session
        .replace_text(&doc, Some(generation), Some(range), &unquote(&new), false)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[given(regex = r#"^the agent keeps the answer's "(/[^"]*)" as "([^"]+)"$"#)]
#[when(regex = r#"^the agent keeps the answer's "(/[^"]*)" as "([^"]+)"$"#)]
#[then(regex = r#"^the agent keeps the answer's "(/[^"]*)" as "([^"]+)"$"#)]
fn keeps(w: &mut StudioWorld, pointer: String, name: String) -> StepResult {
    let value = at(w.answer()?, &pointer)?.clone();
    w.kept.insert(name, value);
    Ok(())
}

// The value is JSON, which never starts with the `a` of "absent".
#[then(regex = r#"^the answer's "(/[^"]*)" is ([^a].*)$"#)]
fn answer_is(w: &mut StudioWorld, pointer: String, want: String) -> StepResult {
    let mut want = json(&want)?;
    w.substitute(&mut want)?;
    let got = at(w.answer()?, &pointer)?;
    (*got == want)
        .then_some(())
        .ok_or_else(|| format!("`{pointer}` is {got}, not {want}"))
}

#[then(regex = r#"^the answer's "(/[^"]*)" is:$"#)]
fn answer_text_is(w: &mut StudioWorld, pointer: String, step: &Step) -> StepResult {
    let want = file_text(step)?;
    let got = at(w.answer()?, &pointer)?;
    // A string is compared as text; anything else as the JSON the docstring
    // holds, so a list of records reads as one.
    let same = match got.as_str() {
        Some(text) => text == want,
        None => {
            serde_json::from_str::<serde_json::Value>(&want).map_err(|e| {
                format!("`{pointer}` is not text, and the docstring is no JSON: {e}")
            })? == *got
        }
    };
    same.then_some(()).ok_or_else(|| {
        let shown = got.as_str().map_or_else(|| got.to_string(), str::to_owned);
        format!("`{pointer}` is:\n{shown}\nnot:\n{want}")
    })
}

#[then(regex = r#"^the answer's "(/[^"]*)" has (\d+) entr(?:y|ies)$"#)]
fn answer_has(w: &mut StudioWorld, pointer: String, count: usize) -> StepResult {
    let got = at(w.answer()?, &pointer)?;
    let len = got
        .as_array()
        .ok_or_else(|| format!("`{pointer}` is not a list: {got}"))?
        .len();
    (len == count)
        .then_some(())
        .ok_or_else(|| format!("`{pointer}` has {len} entries: {got}"))
}

/// One entry of a list is the docstring's JSON, wherever it is in the list:
/// what a scenario says of one member, not of the order of the rest.
#[then(regex = r#"^the answer's "(/[^"]*)" has the entry:$"#)]
fn answer_has_entry(w: &mut StudioWorld, pointer: String, step: &Step) -> StepResult {
    let want: Value = serde_json::from_str(&file_text(step)?)
        .map_err(|e| format!("the docstring is no JSON: {e}"))?;
    let got = at(w.answer()?, &pointer)?;
    let list = got
        .as_array()
        .ok_or_else(|| format!("`{pointer}` is not a list: {got}"))?;
    list.contains(&want)
        .then_some(())
        .ok_or_else(|| format!("`{pointer}` has no entry {want}: {got}"))
}

#[then(regex = r#"^the answer's "(/[^"]*)" is absent$"#)]
fn answer_absent(w: &mut StudioWorld, pointer: String) -> StepResult {
    match w.answer()?.pointer(&pointer) {
        None => Ok(()),
        Some(v) => Err(format!("`{pointer}` is {v}")),
    }
}

#[then(regex = r#"^the answer's "(/[^"]*)" lists "([^"]*)"$"#)]
fn answer_lists(w: &mut StudioWorld, pointer: String, names: String) -> StepResult {
    let got = at(w.answer()?, &pointer)?;
    let want: Vec<Value> = names
        .split(',')
        .map(|n| Value::String(n.trim().to_owned()))
        .filter(|v| v.as_str() != Some(""))
        .collect();
    (got.as_array() == Some(&want))
        .then_some(())
        .ok_or_else(|| format!("`{pointer}` is {got}"))
}

#[then(regex = r#"^the agent is refused with "([a-z_]+)"$"#)]
fn refused(w: &mut StudioWorld, code: String) -> StepResult {
    let error = w.refusal()?;
    (error["code"] == code.as_str())
        .then_some(())
        .ok_or_else(|| format!("refused with {error}"))
}

#[then(regex = r#"^the refusal's "(/[^"]*)" is (.+)$"#)]
fn refusal_is(w: &mut StudioWorld, pointer: String, want: String) -> StepResult {
    let want = json(&want)?;
    let error = w.refusal()?;
    let got = at(&error, &pointer)?;
    (*got == want)
        .then_some(())
        .ok_or_else(|| format!("`{pointer}` is {got}, not {want}"))
}

/// Ljós's messages name a column and a contrast that are its to choose;
/// a scenario pins what it means and leaves those to a pattern.
#[then(regex = r#"^the refusal's message matches "(.+)"$"#)]
fn refusal_matches(w: &mut StudioWorld, pattern: String) -> StepResult {
    let error = w.refusal()?;
    let message = error["message"].as_str().unwrap_or_default();
    let pattern = regex::Regex::new(&pattern).map_err(|e| e.to_string())?;
    pattern
        .is_match(message)
        .then_some(())
        .ok_or_else(|| format!("the message is {message:?}"))
}

#[then(regex = r#"^the file "([^"]+)" is:$"#)]
fn file_is(w: &mut StudioWorld, name: String, step: &Step) -> StepResult {
    let want = file_text(step)?;
    let got =
        std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| format!("`{name}`: {e}"))?;
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("`{name}` is:\n{got}\nnot:\n{want}"))
}

#[then(regex = r"^the history has (\d+) entr(?:y|ies)$")]
fn history_has(w: &mut StudioWorld, want: usize) -> StepResult {
    w.call("history", serde_json::json!({ "action": "list" }))?;
    let answer = w.answer()?;
    let got = answer["done"].as_array().map_or(0, Vec::len);
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("the history has {got} entries, not {want}: {answer}"))
}

#[then(regex = r#"^the file "([^"]+)" contains "(.*)"$"#)]
fn file_contains(w: &mut StudioWorld, name: String, want: String) -> StepResult {
    let want = want.replace("\\\"", "\"");
    let got =
        std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| format!("`{name}`: {e}"))?;
    got.contains(&want)
        .then_some(())
        .ok_or_else(|| format!("`{name}` does not contain {want:?}:\n{got}"))
}

#[then(regex = r#"^the file "([^"]+)" does not contain "(.*)"$"#)]
fn file_lacks(w: &mut StudioWorld, name: String, want: String) -> StepResult {
    let got =
        std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| format!("`{name}`: {e}"))?;
    (!got.contains(&want))
        .then_some(())
        .ok_or_else(|| format!("`{name}` contains {want:?}:\n{got}"))
}

#[then(regex = r#"^the file "([^"]+)" does not exist$"#)]
fn file_absent(w: &mut StudioWorld, name: String) -> StepResult {
    (!w.dir.path().join(&name).exists())
        .then_some(())
        .ok_or_else(|| format!("`{name}` exists"))
}

#[then(regex = r#"^the file "([^"]+)" exists$"#)]
fn file_exists(w: &mut StudioWorld, name: String) -> StepResult {
    w.dir
        .path()
        .join(&name)
        .is_file()
        .then_some(())
        .ok_or_else(|| format!("`{name}` does not exist"))
}

#[given(regex = r#"^the directory "([^"]+)" is deleted$"#)]
fn directory_deleted(w: &mut StudioWorld, name: String) -> StepResult {
    std::fs::remove_dir_all(w.dir.path().join(&name)).map_err(|e| format!("`{name}`: {e}"))
}

/// A new server on the same directory: what a session kept in memory is
/// gone, what it wrote is read again.
#[given("the studio tools are reconnected")]
fn reconnected(w: &mut StudioWorld) -> StepResult {
    w.server = None;
    w.server().map(|_| ())
}

/// The bundle is copied somewhere apart from the project, so it can only
/// run on what it holds, and started as its manifest says.
#[then(
    regex = r#"^the bundle "([^"]+)", copied out of the project, runs headless and shows what the preview shows$"#
)]
fn bundle_runs(w: &mut StudioWorld, out: String) -> StepResult {
    use studio_core::preview::{PreviewHost, PreviewSpec, Target};

    w.answer()?;
    let want = w.preview_outline()?;
    let copy = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_dir(&w.dir.path().join(&out), copy.path())?;
    let manifest: studio_mcp::export::Manifest = serde_json::from_str(
        &std::fs::read_to_string(copy.path().join(studio_mcp::export::MANIFEST))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let state = tempfile::tempdir().map_err(|e| e.to_string())?;
    // A directory for the host to make: a temporary directory itself is as
    // open as the umask leaves it.
    let mut host = NativeHost::new(
        copy.path(),
        NativeHost::default_program(),
        state.path().join("preview"),
    );
    host.open(&PreviewSpec {
        entry: manifest.entry,
        // What `preview_open` runs at when no size is given.
        size: Target::Desktop.size(),
        aliases: Vec::new(),
        types: Vec::new(),
    })
    .map_err(|e| format!("the bundle does not start: {e}"))?;
    let got = host.outline().map_err(|e| e.to_string())?;
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("the bundle shows:\n{got}\nthe preview:\n{want}"))
}

#[then(regex = r#"^the answer's "(/[^"]*)" names the ljos-web commit Studio is pinned to$"#)]
fn names_pinned_ljos_web(w: &mut StudioWorld, pointer: String) -> StepResult {
    // The pin's one place, read as the tasks and CI read it.
    let rev = std::fs::read_to_string(
        std::path::Path::new(&manifest_dir()).join("../../web/ljos-web.rev"),
    )
    .map_err(|e| e.to_string())?;
    let got = at(w.answer()?, &pointer)?;
    (got == rev.trim())
        .then_some(())
        .ok_or_else(|| format!("{got} is not the pin {}", rev.trim()))
}

/// The site bundle is copied out of the project, so the builder can only
/// build what it holds.
#[then(regex = r#"^a site builder held to the contract builds "([^"]+)", starting at "([^"]+)"$"#)]
fn site_builds(w: &mut StudioWorld, out: String, entry: String) -> StepResult {
    w.answer()?;
    let copy = tempfile::tempdir().map_err(|e| e.to_string())?;
    copy_dir(&w.dir.path().join(&out), copy.path())?;
    let site = site::build(copy.path(), &entry)?;
    site.documents
        .contains_key(&site.entry)
        .then_some(())
        .ok_or_else(|| format!("the site compiles no `{}`: {site:?}", site.entry))
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> StepResult {
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        // A bundle's links are relative to it, so they hold in the copy.
        if kind.is_symlink() {
            let link = std::fs::read_link(entry.path()).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, &target).map_err(|e| e.to_string())?;
            #[cfg(not(unix))]
            return Err(format!("{}: links are copied on unix only", link.display()));
        } else if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[then(regex = r#"^the agent is offered the tools "([^"]*)"$"#)]
fn offered(w: &mut StudioWorld, names: String) -> StepResult {
    let caps = w.caps;
    let got: Vec<&str> = w.server()?.tools().visible(caps).map(|d| d.name).collect();
    let want: Vec<&str> = names
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("offered {got:?}"))
}

/// A reload Studio reported, applied or not.
fn is_reload(event: &Value) -> bool {
    matches!(event["kind"].as_str(), Some("reloaded" | "reload_failed"))
}

#[then(regex = r#"^the preview shows "([^"]*)"$"#)]
fn preview_shows(w: &mut StudioWorld, text: String) -> StepResult {
    // Read first: what a burst of writes left out is reloaded before the
    // events answer, so the outline is read after every reload they name.
    let events = w.preview_events()?;
    let outline = w.preview_outline()?;
    let (reloads, lines) = StudioWorld::reloads(&events);
    w.reloads_read = reloads;
    outline
        .contains(&format!("\"{text}\""))
        .then_some(())
        .ok_or_else(|| {
            format!("the preview shows:\n{outline}\nafter these events of Studio's:\n{lines}")
        })
}

// What a write is waited on by: the reload Studio reports for it, which it
// asks of the preview before the write's answer. Not a time.
#[then(regex = r"^Studio has reported the preview reloaded since it was read$")]
fn reloaded_since_read(w: &mut StudioWorld) -> StepResult {
    let events = w.preview_events()?;
    let (reloads, lines) = StudioWorld::reloads(&events);
    let read = w.reloads_read;
    let last = events.iter().rev().find(|e| is_reload(e));
    match last {
        Some(event) if reloads > read && event["kind"] == "reloaded" => Ok(()),
        Some(event) if reloads > read => Err(format!(
            "the last reload Studio reported failed: {event}\nevents:\n{lines}"
        )),
        _ => Err(format!(
            "no reload since the preview was read ({read} then, {reloads} now; the last at \
             generation {}); events:\n{lines}",
            last.and_then(|e| e["generation"].as_u64())
                .map_or_else(|| "none".to_owned(), |g| g.to_string())
        )),
    }
}

/// A file written outside Studio reaches the preview through the preview's
/// own watch, which runs after the write returns, so the outline is read
/// again until the deadline.
#[then(regex = r#"^the preview shows "([^"]*)" within (\d+) seconds$"#)]
fn preview_shows_within(w: &mut StudioWorld, text: String, seconds: u64) -> StepResult {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    loop {
        let outline = w.preview_outline()?;
        if outline.contains(&format!("\"{text}\"")) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "after {seconds} seconds the preview shows:\n{outline}"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[then(regex = r#"^the answer has a PNG picture at most (\d+) pixels on its longest side$"#)]
fn png_picture(w: &mut StudioWorld, side: u32) -> StepResult {
    w.answer()?;
    let [image] = w.images.as_slice() else {
        return Err(format!("{} pictures", w.images.len()));
    };
    if image.mime != "image/png" {
        return Err(format!("a picture of type {}", image.mime));
    }
    let (width, height) = png_size(&image.data).ok_or("the picture is not a PNG")?;
    (width.max(height) <= side && width > 0 && height > 0)
        .then_some(())
        .ok_or_else(|| format!("the picture is {width}x{height}"))
}

/// Width and height from a PNG's header chunk, which comes first.
fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !data.starts_with(SIGNATURE) || data.get(12..16)? != b"IHDR" {
        return None;
    }
    let word = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
    Some((word(16)?, word(20)?))
}

/// Serves agents as a running Studio does, keeping the endpoint or the
/// error it answered instead.
fn start_serving(w: &mut StudioWorld, caps: &str) -> Result<Result<(), String>, String> {
    let caps = CapSet::parse_list(caps).map_err(|e| e.to_string())?;
    let studio = Studio::open(w.dir.path()).map_err(|e| e.to_string())?;
    let server = Arc::new(StudioServer::new(studio));
    Ok(
        match endpoint::serve_agents(&server, caps, &w.endpoint_state()) {
            Ok(http) => {
                w.endpoint = Some(http);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        },
    )
}

#[given(regex = r#"^Studio serves agents with the capabilities "([^"]*)"$"#)]
fn serves_agents(w: &mut StudioWorld, caps: String) -> StepResult {
    start_serving(w, &caps)?
}

#[given("the directory for the endpoint's token can be entered by others")]
fn token_dir_open(w: &mut StudioWorld) -> StepResult {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = w.endpoint_state();
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())
}

#[when(regex = r#"^Studio starts serving agents with the capabilities "([^"]*)"$"#)]
fn starts_serving(w: &mut StudioWorld, caps: String) -> StepResult {
    w.not_served = start_serving(w, &caps)?.err();
    Ok(())
}

#[then(regex = r#"^Studio cannot serve agents, saying "([^"]+)"$"#)]
fn cannot_serve(w: &mut StudioWorld, said: String) -> StepResult {
    if w.endpoint.is_some() {
        return Err("it serves agents".to_owned());
    }
    let why = w.not_served.as_deref().ok_or("no error was answered")?;
    why.contains(&said)
        .then_some(())
        .ok_or_else(|| format!("it said: {why}"))
}

#[then("no token file is written")]
fn no_token_file(w: &mut StudioWorld) -> StepResult {
    let path = w.endpoint_state().join(endpoint::TOKEN_FILE);
    (!path.exists())
        .then_some(())
        .ok_or_else(|| format!("{} exists", path.display()))
}

#[when(
    regex = r#"^a client calls the tool "([^"]+)" on the endpoint( with the token| with another token)?(?: and the (Origin|Host) "([^"]+)")?$"#
)]
fn client_calls(
    w: &mut StudioWorld,
    tool: String,
    token: String,
    header: String,
    value: String,
) -> StepResult {
    use std::io::{Read as _, Write as _};
    let http = w.endpoint.as_ref().ok_or("nothing serves agents")?;
    let addr = http.addr().ok_or("the endpoint has no address")?;
    let mut head = vec![
        "POST /mcp HTTP/1.1".to_owned(),
        "Connection: close".to_owned(),
        "Accept: application/json, text/event-stream".to_owned(),
        "Content-Type: application/json".to_owned(),
    ];
    head.push(if header == "Host" {
        format!("Host: {value}")
    } else {
        format!("Host: {addr}")
    });
    if header == "Origin" {
        head.push(format!("Origin: {value}"));
    }
    match token.trim() {
        "with the token" => head.push(format!("Authorization: Bearer {}", http.token().as_str())),
        "with another token" => head.push(format!("Authorization: Bearer {}", "0".repeat(64))),
        _ => {}
    }
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool, "arguments": {} },
    })
    .to_string();
    head.push(format!("Content-Length: {}", body.len()));
    let request = format!("{}\r\n\r\n{body}", head.join("\r\n"));
    let mut stream = std::net::TcpStream::connect(addr).map_err(|e| e.to_string())?;
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    stream
        .read_to_string(&mut answer)
        .map_err(|e| e.to_string())?;
    let (head, body) = answer
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("no HTTP answer: {answer}"))?;
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("no status in {head}"))?;
    w.reply = Some((status, body.to_owned()));
    Ok(())
}

/// The JSON in the endpoint's last answer, which may come chunked.
fn reply_json(w: &StudioWorld) -> Result<(u16, Value), String> {
    let (status, body) = w.reply.as_ref().ok_or("the endpoint was not called")?;
    let start = body.find('{').ok_or_else(|| format!("no JSON in {body}"))?;
    let end = body
        .rfind('}')
        .ok_or_else(|| format!("no JSON in {body}"))?;
    let value = serde_json::from_str(&body[start..=end]).map_err(|e| format!("{e}: {body}"))?;
    Ok((*status, value))
}

#[then("the endpoint answers it")]
fn endpoint_answers(w: &mut StudioWorld) -> StepResult {
    let (status, value) = reply_json(w)?;
    (status == 200 && value["result"]["isError"] != true)
        .then_some(())
        .ok_or_else(|| format!("{status}: {value}"))
}

#[then(regex = r#"^the endpoint refuses it with (\d+) "([a-z_]+)"$"#)]
fn endpoint_refuses(w: &mut StudioWorld, want: u16, code: String) -> StepResult {
    let (status, value) = reply_json(w)?;
    (status == want && value["error"]["code"] == code.as_str())
        .then_some(())
        .ok_or_else(|| format!("{status}: {value}"))
}

#[then(regex = r#"^the endpoint's tool refuses it with "([a-z_]+)"$"#)]
fn endpoint_tool_refuses(w: &mut StudioWorld, code: String) -> StepResult {
    let (status, value) = reply_json(w)?;
    (status == 200
        && value["result"]["isError"] == true
        && value["result"].to_string().contains(&code))
    .then_some(())
    .ok_or_else(|| format!("{status}: {value}"))
}

#[then("the endpoint listens on loopback only")]
fn on_loopback(w: &mut StudioWorld) -> StepResult {
    let http = w.endpoint.as_ref().ok_or("nothing serves agents")?;
    let addr = http.addr().ok_or("the endpoint has no address")?;
    addr.ip()
        .is_loopback()
        .then_some(())
        .ok_or_else(|| format!("it listens on {addr}"))
}

#[then("the token file can be read by its owner alone")]
fn token_file_private(w: &mut StudioWorld) -> StepResult {
    use std::os::unix::fs::PermissionsExt as _;
    let http = w.endpoint.as_ref().ok_or("nothing serves agents")?;
    let path = w.endpoint_state().join(endpoint::TOKEN_FILE);
    let meta = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mode = meta.permissions().mode() & 0o777;
    (mode == 0o600 && text.contains(http.token().as_str()))
        .then_some(())
        .ok_or_else(|| format!("mode {mode:o}, or another token in it"))
}

/// The crate's directory as cargo names it to the running test: the shared
/// compile cache may hand a binary built in another clone of the
/// repository, whose `env!` path is that clone's.
fn manifest_dir() -> String {
    std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned())
}

#[tokio::main]
async fn main() {
    let features = format!("{}/../../features", manifest_dir());
    let input = std::env::var("STUDIO_FEATURES").unwrap_or(features);
    StudioWorld::cucumber()
        // A preview is a whole application in a process of its own.
        .max_concurrent_scenarios(8)
        .fail_on_skipped()
        // A window's scenarios run in studio_blocks, which drives the shell;
        // the browser's storage only in a browser's build (`web/features.mjs`).
        .filter_run_and_exit(input, |feature, _, scenario| {
            !feature
                .tags
                .iter()
                .chain(&scenario.tags)
                .any(|t| t == "ui" || t == "web")
        })
        .await;
}
