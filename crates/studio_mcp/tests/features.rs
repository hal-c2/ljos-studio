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

use std::collections::HashMap;

use cucumber::{World, gherkin::Step, given, then, when};
use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface, SurfaceError};
use serde_json::Value;
use studio_mcp::{Studio, StudioServer};

#[path = "support/schema.rs"]
mod schema;

#[derive(World)]
#[world(init = Self::new)]
struct StudioWorld {
    dir: tempfile::TempDir,
    /// Directories links lead out of the project to.
    outside: Vec<tempfile::TempDir>,
    server: Option<StudioServer>,
    caps: CapSet,
    answer: Option<Result<Value, SurfaceError>>,
    /// Values kept from answers, written `$name` in later arguments.
    kept: HashMap<String, Value>,
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
            server: None,
            caps: CapSet::of([Cap::Inspect, Cap::Mutate]),
            answer: None,
            kept: HashMap::new(),
        }
    }

    fn server(&mut self) -> Result<&StudioServer, String> {
        if self.server.is_none() {
            let studio = Studio::open(self.dir.path()).map_err(|e| e.to_string())?;
            self.server = Some(StudioServer::new(studio));
        }
        Ok(self.server.as_ref().expect("just made"))
    }

    fn call(&mut self, tool: &str, args: Value) -> StepResult {
        let caps = self.caps;
        let server = self.server()?;
        let (out, def) = server.tools().call(tool, &CallCtx::new(caps), args);
        // A refusal is an error result, which the output schema does not cover.
        if let (Ok(answer), Some(output)) = (&out, def.and_then(|d| d.output_schema.as_deref())) {
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
        Ok(())
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
    std::os::unix::fs::symlink(outside.path(), w.dir.path().join(name))
        .map_err(|e| e.to_string())?;
    w.outside.push(outside);
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
    // A `Given` sets the scene: a refusal there is the scenario's mistake.
    if step.keyword.trim() == "Given" {
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
    (got.as_str() == Some(want.as_str()))
        .then_some(())
        .ok_or_else(|| {
            format!(
                "`{pointer}` is:\n{}\nnot:\n{want}",
                got.as_str().unwrap_or("")
            )
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

#[then(regex = r#"^the file "([^"]+)" is:$"#)]
fn file_is(w: &mut StudioWorld, name: String, step: &Step) -> StepResult {
    let want = file_text(step)?;
    let got =
        std::fs::read_to_string(w.dir.path().join(&name)).map_err(|e| format!("`{name}`: {e}"))?;
    (got == want)
        .then_some(())
        .ok_or_else(|| format!("`{name}` is:\n{got}\nnot:\n{want}"))
}

#[then(regex = r#"^the file "([^"]+)" does not exist$"#)]
fn file_absent(w: &mut StudioWorld, name: String) -> StepResult {
    (!w.dir.path().join(&name).exists())
        .then_some(())
        .ok_or_else(|| format!("`{name}` exists"))
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

#[tokio::main]
async fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features");
    let input = std::env::var("STUDIO_FEATURES").unwrap_or_else(|_| features.to_owned());
    StudioWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(input)
        .await;
}
