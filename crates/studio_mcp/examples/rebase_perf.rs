//! Times what rebasing costs (DESIGN.md "Phase 4: rebase by handle, as
//! built"): on a 1,000-line document another writer has made 50 edits to
//! since an agent read it, the agent's edit made at that old generation, by
//! handle (`property_set`) and by text (`document_edit`), against the same
//! edit made at the current generation. Each is a `dry_run`, so the
//! document stays as it is between samples and every sample crosses the
//! same 50 entries. Then the bytes the log keeps per entry.
//!
//! `cargo run --release -p studio_mcp --example rebase_perf [-- CALLS]`

use std::time::{Duration, Instant};

use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer};

/// Four lines an object, with the root's three.
const OBJECTS: usize = 249;
const ENTRIES: usize = 50;

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
    let mut text = String::from("import QtQuick\n\nItem {\n");
    for i in 0..OBJECTS {
        text.push_str(&format!(
            "    Rectangle {{\n        id: r{i}\n        width: {i}\n    }}\n"
        ));
    }
    text.push_str("}\n");
    assert_eq!(text.lines().count(), 3 + OBJECTS * 4 + 1);
    std::fs::write(dir.path().join("Main.qml"), &text).unwrap();

    let server = StudioServer::new(Studio::open(dir.path()).unwrap());
    let caps = CapSet::default().with(Cap::Inspect).with(Cap::Mutate);
    let agent = CallCtx::new(caps);
    let other = CallCtx {
        client: Some("other".to_owned()),
        ..CallCtx::new(caps)
    };
    let call = |ctx: &CallCtx, tool: &str, args: Value| -> Value {
        match server.call(tool, ctx, args) {
            Ok(reply) => reply.structured,
            Err(e) => panic!("{tool}: {e:?}"),
        }
    };
    let base = call(&agent, "document_tree", json!({"doc": "Main.qml"}))["generation"]
        .as_u64()
        .unwrap();
    // The other writer's edits land in the first objects, the agent's in
    // the last, so nothing conflicts and every entry is crossed.
    for i in 0..ENTRIES {
        call(
            &other,
            "property_set",
            json!({"doc": "Main.qml", "node": {"id": format!("r{i}")}, "name": "width", "value": 1000 + i}),
        );
    }
    let current = call(&agent, "document_tree", json!({"doc": "Main.qml"}))["generation"]
        .as_u64()
        .unwrap();
    assert_eq!(current, base + ENTRIES as u64);

    let last = OBJECTS - 1;
    let set = |generation: u64| {
        json!({"doc": "Main.qml", "generation": generation, "node": {"id": format!("r{last}")},
               "name": "width", "value": 7, "dry_run": true})
    };
    // The bytes of the last object's `width: 248` (ten) as the agent read them:
    // past every other writer's edit, so the transform moves it.
    let at = text.find(&format!("width: {last}")).unwrap();
    let typed = |generation: u64, shift: usize| {
        json!({"doc": "Main.qml", "generation": generation, "dry_run": true,
               "start": at + shift, "end": at + shift + 10, "text": "width: 7"})
    };
    // Where those bytes are now, for the same edit made at the current
    // generation.
    let shift = text_now(&call, &agent)
        .find(&format!("width: {last}"))
        .unwrap()
        - at;

    let time = |name: &str, ctx: &CallCtx, tool: &str, args: &Value, rebased: bool| {
        let mut samples = Vec::with_capacity(calls);
        for _ in 0..calls {
            let started = Instant::now();
            let out = call(ctx, tool, args.clone());
            samples.push(started.elapsed());
            assert_eq!(
                out["rebased_from"].as_u64().is_some(),
                rebased,
                "{name}: {out}"
            );
        }
        report(name, samples);
    };
    time(
        "property_set at the current generation",
        &agent,
        "property_set",
        &set(current),
        false,
    );
    time(
        &format!("property_set rebased over {ENTRIES} entries"),
        &agent,
        "property_set",
        &set(base),
        true,
    );
    time(
        "document_edit at the current generation",
        &agent,
        "document_edit",
        &typed(current, shift),
        false,
    );
    time(
        &format!("document_edit rebased over {ENTRIES} entries"),
        &agent,
        "document_edit",
        &typed(base, 0),
        true,
    );

    let studio = server.studio();
    let (bytes, entries) = studio.lock().unwrap().session.log_size();
    println!(
        "log: {entries} entries, {bytes} bytes, {} bytes an entry",
        bytes / entries.max(1)
    );
}

fn text_now(call: &impl Fn(&CallCtx, &str, Value) -> Value, ctx: &CallCtx) -> String {
    call(ctx, "document_text", json!({"doc": "Main.qml"}))["text"]
        .as_str()
        .unwrap()
        .to_owned()
}
