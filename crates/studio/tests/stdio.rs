//! The whole server through its real front door: the `studio` program on
//! stdio, with a real `ljos` preview.
//!
//! Tools answer on a current-thread runtime, and the preview host's client
//! blocks. These are safe together because that client does its work on its
//! own thread with its own runtime, and only the waiting happens here; this
//! test is what holds that, since a nested runtime panics and a deadlock
//! times out.
//!
//! It needs `LJOS`: `mise run features` builds it, sets it and runs this.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::fs::MetadataExt,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, channel},
    time::Duration,
};

use serde_json::{Value, json};

/// A cold preview compiles the whole project before it answers.
const ANSWER_LIMIT: Duration = Duration::from_secs(60);

struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl Server {
    fn start(project: &std::path::Path, runtime: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_studio"))
            .args(["--mcp", "--caps", "inspect,mutate,run", "--ljos"])
            .arg(std::env::var_os("LJOS").expect("LJOS names the ljos program"))
            .arg(project)
            .env("XDG_RUNTIME_DIR", runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the studio program runs");
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let (send, lines) = channel();
        // On a thread so that a server that hangs fails the test by time.
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            lines,
            next_id: 0,
        };
        server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "stdio-test", "version": "0" },
            }),
        );
        server.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        server
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("the server is reading");
        self.stdin.flush().expect("the server is reading");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let line = self
                .lines
                .recv_timeout(ANSWER_LIMIT)
                .unwrap_or_else(|e| panic!("no answer to `{method}`: {e}"));
            let answer: Value = serde_json::from_str(&line).expect("a JSON-RPC line");
            if answer["id"] == id {
                assert!(answer.get("error").is_none(), "{method}: {answer}");
                return answer["result"].clone();
            }
        }
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        assert_ne!(result["isError"], true, "{name}: {result}");
        result
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "needs LJOS: run by `mise run features`"]
fn a_preview_opens_and_answers_through_the_stdio_server() {
    let project = tempfile::tempdir().unwrap();
    let runtime = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("Main.qml"),
        "import QtQuick\n\nRectangle {\n    width: 200\n    height: 100\n    color: \"red\"\n}\n",
    )
    .unwrap();
    let mut server = Server::start(project.path(), runtime.path());

    let opened = server.tool(
        "preview_open",
        json!({ "doc": "Main.qml", "target": "phone" }),
    );
    assert_eq!(opened["structuredContent"]["started"], true, "{opened}");

    let picture = server.tool("preview_screenshot", json!({ "max_size": 64 }));
    assert!(
        picture["structuredContent"]["bytes"].as_u64().unwrap() > 0,
        "{picture}"
    );
    let image = picture["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "image")
        .unwrap_or_else(|| panic!("no image in {picture}"));
    assert_eq!(image["mimeType"], "image/png");

    // A write reloads through the same client before the tool answers.
    server.tool(
        "binding_set",
        json!({ "doc": "Main.qml", "generation": 1, "node": { "path": "root" }, "name": "color", "value": "\"blue\"" }),
    );
    let events = server.tool("preview_events", json!({}));
    let kinds: Vec<&str> = events["structuredContent"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"reloaded"), "{events}");
    assert_eq!(events["structuredContent"]["open"], true, "{events}");

    // The token lives in a directory this user alone can reach.
    let state = runtime
        .path()
        .join(format!("ljos-studio-{}", server.child.id()));
    let found = state.symlink_metadata().unwrap();
    assert_eq!(found.mode() & 0o777, 0o700);
}

/// The programs `pid` started.
fn children(pid: u32) -> Vec<u32> {
    let mut found = Vec::new();
    for task in std::fs::read_dir(format!("/proc/{pid}/task"))
        .unwrap()
        .flatten()
    {
        let text = std::fs::read_to_string(task.path().join("children")).unwrap_or_default();
        found.extend(
            text.split_whitespace()
                .filter_map(|c| c.parse::<u32>().ok()),
        );
    }
    found
}

fn stopped_by(signal: &str) {
    let project = tempfile::tempdir().unwrap();
    let runtime = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("Main.qml"),
        "import QtQuick\n\nRectangle {\n    width: 200\n    height: 100\n}\n",
    )
    .unwrap();
    let mut server = Server::start(project.path(), runtime.path());
    server.tool(
        "preview_open",
        json!({ "doc": "Main.qml", "target": "phone" }),
    );
    let studio = server.child.id();
    let previews = children(studio);
    assert_eq!(previews.len(), 1, "{previews:?}");
    let state = runtime.path().join(format!("ljos-studio-{studio}"));
    assert!(state.is_dir());

    let sent = Command::new("kill")
        .args([signal, &studio.to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    server.child.wait().unwrap();

    let preview = std::path::PathBuf::from(format!("/proc/{}", previews[0]));
    let alive = preview.exists();
    if alive {
        // Not to leave one behind when this test fails.
        let _ = Command::new("kill").arg(previews[0].to_string()).status();
    }
    assert!(!alive, "the preview outlived Studio");
    assert!(!state.exists(), "the state directory outlived Studio");
}

#[test]
#[ignore = "needs LJOS: run by `mise run features`"]
fn sigterm_takes_the_preview_down_with_studio() {
    stopped_by("-TERM");
}

#[test]
#[ignore = "needs LJOS: run by `mise run features`"]
fn sigint_takes_the_preview_down_with_studio() {
    stopped_by("-INT");
}
