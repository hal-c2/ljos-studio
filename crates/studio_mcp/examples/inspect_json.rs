//! Times the tools phase 3 / M1 changed (DESIGN.md), as an agent calls them
//! on a project whose list reads a JSON file of 1,000 records:
//! `node_inspect` of the list (its roles are the records' keys, read from
//! the file each call) and of a `Button` (its signals, from Ljós's table of
//! types and the project's type dump), `state_edit` writing a handler for a
//! signal the type is checked to have, and `document_new` writing a
//! `JsonListModel` document for the file.
//!
//! `cargo run --release -p studio_mcp --example inspect_json [-- CALLS]`

use std::{
    fmt::Write as _,
    path::Path,
    time::{Duration, Instant},
};

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer};

const RECORDS: usize = 1000;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn report(name: &str, mut samples: Vec<Duration>) {
    samples.sort();
    let at = |p: f64| samples[((samples.len() - 1) as f64 * p).round() as usize];
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "{name}: p50 {:.3} ms, p95 {:.3} ms, max {:.3} ms over {}",
        ms(at(0.5)),
        ms(at(0.95)),
        ms(at(1.0)),
        samples.len()
    );
}

fn main() {
    let calls: usize = std::env::args()
        .nth(1)
        .map_or(200, |n| n.parse().expect("CALLS is a number"));
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut records = String::from("[");
    for i in 0..RECORDS {
        if i > 0 {
            records.push(',');
        }
        write!(
            records,
            r#"{{"name": "Person {i}", "team": "Team {}", "room": {i}, "phone": "555-{i:04}"}}"#,
            i % 7
        )
        .unwrap();
    }
    records.push(']');
    write(root, "data/people.json", &records);
    write(
        root,
        "App/Data/qmldir",
        "module App.Data\nPeople 1.0 People.qml\n",
    );
    write(
        root,
        "App/Data/People.qml",
        "import Ljos.Models\n\nJsonListModel {\n    source: Qt.resolvedUrl(\"../../data/people.json\")\n}\n",
    );
    // A dump, as a block written in Rust leaves one: looked up for every
    // type the documents name.
    write(
        root,
        "native/ljostypes.json",
        r#"{"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
          {"name": "Player", "since": "1.0", "base": "QtObject", "baseModule": "QtQml",
           "creatable": true, "defaultProperty": null, "attached": null,
           "properties": [], "signals": [{"name": "finished", "parameters": []}],
           "methods": []}]}]}"#,
    );
    write(
        root,
        "Main.qml",
        "import QtQuick\nimport QtQuick.Controls\nimport App.Data\nimport App.Media\n\n\
         Item {\n    id: root\n    states: [\n        State {\n            name: \"details\"\n        }\n    ]\n\n    \
         ListView {\n        id: list\n        model: People {}\n        delegate: Text {\n            \
         text: model.name\n        }\n    }\n    Button {\n        id: back\n    }\n    \
         Player {\n        id: player\n    }\n}\n",
    );

    let server = StudioServer::new(Studio::open(root).unwrap());
    let ctx = CallCtx::new(CapSet::default().with(Cap::Inspect).with(Cap::Mutate));
    let call = |tool: &str, args: Value| -> Value {
        let (out, _) = server.tools().call(tool, &ctx, args);
        out.unwrap_or_else(|e| panic!("{tool} was refused: {}", e.to_value()))
    };
    let timed = |tool: &str, args: Value, samples: &mut Vec<Duration>| -> Value {
        let started = Instant::now();
        let out = call(tool, args);
        samples.push(started.elapsed());
        out
    };

    let mut samples = Vec::new();
    for _ in 0..calls {
        let out = timed(
            "node_inspect",
            json!({"doc": "Main.qml", "node": {"id": "list"}}),
            &mut samples,
        );
        assert_eq!(out["model"]["rows"], RECORDS);
    }
    report(
        &format!(
            "node_inspect of a list of {RECORDS} JSON records ({} bytes)",
            records.len()
        ),
        samples,
    );

    let mut samples = Vec::new();
    for _ in 0..calls {
        let out = timed(
            "node_inspect",
            json!({"doc": "Main.qml", "node": {"id": "back"}}),
            &mut samples,
        );
        assert!(out["signals"].as_array().is_some_and(|s| !s.is_empty()));
    }
    report("node_inspect of a Button, with its signals", samples);

    let mut samples = Vec::new();
    for _ in 0..calls {
        let out = timed(
            "node_inspect",
            json!({"doc": "Main.qml", "node": {"id": "player"}}),
            &mut samples,
        );
        let signals = out["signals"].as_array().unwrap();
        assert!(signals.iter().any(|s| s["name"] == "finished"));
    }
    report("node_inspect of a dumped type, with its signals", samples);

    let mut samples = Vec::new();
    for _ in 0..calls {
        timed(
            "state_edit",
            json!({"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"},
                   "signal": "clicked", "state": "details"}),
            &mut samples,
        );
        call(
            "state_edit",
            json!({"doc": "Main.qml", "action": "remove_interaction", "trigger": {"id": "back"},
                   "signal": "clicked"}),
        );
    }
    report("state_edit interaction, its signal checked", samples);

    let mut samples = Vec::new();
    for i in 0..calls {
        timed(
            "document_new",
            json!({"doc": format!("lists/List{i}.qml"), "data": "data/people.json"}),
            &mut samples,
        );
    }
    report(
        &format!("document_new of a JsonListModel for {RECORDS} records"),
        samples,
    );
}
