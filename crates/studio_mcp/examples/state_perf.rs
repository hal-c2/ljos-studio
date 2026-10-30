//! Times `state_edit` as an agent calls it: 50 edits of a document of 200
//! objects, each kind in turn (add a state, change a value in it, give it a
//! breakpoint, add a transition into it, rename it), so the states list
//! grows as a real one does and every edit splices a list of them.
//!
//! `cargo run --release -p studio_mcp --example state_perf [-- EDITS]`

use std::time::{Duration, Instant};

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer};

const OBJECTS: usize = 200;
const DOC: &str = "Main.qml";

fn main() {
    let edits: usize = std::env::args()
        .nth(1)
        .map_or(50, |n| n.parse().expect("EDITS is a number"));
    let dir = tempfile::tempdir().unwrap();
    let mut text = String::from("import QtQuick\n\nItem {\n    id: root\n");
    for i in 1..OBJECTS {
        text += &format!(
            "\n    Rectangle {{\n        id: r{i}\n        x: {i}; width: 10; height: 10\n        color: \"grey\"\n    }}\n"
        );
    }
    text += "}\n";
    std::fs::write(dir.path().join(DOC), &text).unwrap();
    let server = StudioServer::new(Studio::open(dir.path()).unwrap());
    let ctx = CallCtx::new(CapSet::default().with(Cap::Inspect).with(Cap::Mutate));
    let call = |args: Value| -> Duration {
        let started = Instant::now();
        let (out, _) = server.tools().call("state_edit", &ctx, args);
        let took = started.elapsed();
        if let Err(e) = out {
            panic!("state_edit was refused: {}", e.to_value());
        }
        took
    };
    // The document is read once before timing, as an agent's first
    // `document_tree` would, in the pages the server answers in.
    let mut objects = 0;
    let mut cursor = Value::Null;
    loop {
        let (page, _) = server.tools().call(
            "document_tree",
            &ctx,
            json!({ "doc": DOC, "cursor": cursor }),
        );
        let page = page.unwrap();
        objects += page["nodes"].as_array().unwrap().len();
        // A page the size budget cut short carries its cursor in
        // `next_cursor` rather than `next`.
        match page
            .get("next")
            .or(page.get("next_cursor"))
            .unwrap_or(&Value::Null)
        {
            Value::String(next) => cursor = Value::String(next.clone()),
            _ => break,
        }
    }
    assert_eq!(objects, OBJECTS);

    let mut samples = Vec::new();
    for i in 0..edits {
        let state = format!("s{}", i / 5);
        let args = match i % 5 {
            0 => json!({ "doc": DOC, "action": "add", "state": state }),
            1 => json!({
                "doc": DOC, "action": "change", "state": state,
                "target": { "id": format!("r{}", 1 + i % (OBJECTS - 1)) },
                "name": "width", "value": i,
            }),
            2 => json!({ "doc": DOC, "action": "when", "state": state, "breakpoint": "phone" }),
            3 => json!({
                "doc": DOC, "action": "add_transition", "to": state,
                "animations": "NumberAnimation { properties: \"width\"; duration: 120 }",
            }),
            _ => json!({
                "doc": DOC, "action": "rename", "state": state,
                "new_name": format!("t{}", i / 5),
            }),
        };
        samples.push(call(args));
    }
    samples.sort();
    let at = |p: f64| samples[((samples.len() - 1) as f64 * p).round() as usize];
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "state_edit on {OBJECTS} objects: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms over {}",
        ms(at(0.5)),
        ms(at(0.95)),
        ms(at(1.0)),
        samples.len()
    );
}
