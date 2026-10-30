//! Times what a person feels: opening every document of a project and one
//! edit in its largest file, through the same tools an agent calls.
//!
//! `cargo run --release -p studio_mcp --example perf -- <project dir>`
//!
//! The edit is made in a copy of the project: the timing must not depend on
//! who else may write the original.

use std::{path::Path, time::Instant};

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer};

fn main() {
    let source = std::env::args().nth(1).expect("usage: perf <project dir>");
    let source = Path::new(&source);
    let mut files = Vec::new();
    collect(source, source, &mut files);
    files.sort();
    let copy = tempfile::tempdir().unwrap();
    let dir = copy.path();
    for file in &files {
        let to = dir.join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(source.join(file), to).unwrap();
    }
    let caps = CapSet::default().with(Cap::Inspect).with(Cap::Mutate);
    let ctx = CallCtx::new(caps);
    let started = Instant::now();
    let server = StudioServer::new(Studio::open(dir).unwrap());
    let call = |tool: &str, args: Value| -> Value {
        let (out, _) = server.tools().call(tool, &ctx, args);
        out.unwrap_or_else(|e| e.to_value())
    };
    let mut lines = 0;
    let mut largest = (0, String::new());
    let mut nodes = 0;
    for file in &files {
        let text = std::fs::read_to_string(dir.join(file)).unwrap_or_default();
        lines += text.lines().count();
        largest = largest.max((text.len(), file.clone()));
        nodes += whole_tree(&call, file).len();
    }
    let opened = started.elapsed();
    let doc = largest.1;
    let tree = call("document_tree", json!({ "doc": doc, "limit": 100000 }));
    let last = tree["nodes"]
        .as_array()
        .and_then(|n| n.last())
        .cloned()
        .unwrap();
    let doc_lines = std::fs::read_to_string(dir.join(&doc))
        .unwrap()
        .lines()
        .count();
    let edit = Instant::now();
    let out = call(
        "property_set",
        json!({ "doc": doc, "node": { "node": last["node"] }, "name": "opacity", "value": 0.5 }),
    );
    let edited = edit.elapsed();
    assert!(out.get("error").is_none(), "{out}");
    let undo = Instant::now();
    call("history", json!({ "action": "undo" }));
    let undone = undo.elapsed();
    println!(
        "{} documents, {lines} lines, {nodes} objects: opened in {:.1} ms",
        files.len(),
        opened.as_secs_f64() * 1e3
    );
    println!(
        "property_set in {doc} ({doc_lines} lines): {:.2} ms; undo {:.2} ms",
        edited.as_secs_f64() * 1e3,
        undone.as_secs_f64() * 1e3
    );
}

fn whole_tree(call: &impl Fn(&str, Value) -> Value, doc: &str) -> Vec<Value> {
    let mut nodes = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let page = call("document_tree", json!({ "doc": doc, "cursor": cursor }));
        nodes.extend(page["nodes"].as_array().cloned().unwrap_or_default());
        match &page["next"] {
            Value::String(next) => cursor = Value::String(next.clone()),
            _ => return nodes,
        }
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "qml") {
            let rel = path.strip_prefix(root).unwrap();
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}
