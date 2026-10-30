//! Times the edit-to-preview path (DESIGN.md section 6: ≤ 50 ms p95): a
//! preview of a document runs, and each edit an agent makes is written and
//! shown before its tool call answers.
//!
//! `cargo run --release -p studio_mcp --example preview_perf -- <project dir> <doc> [edits]`
//!
//! `LJOS` names the `ljos` program. The edits toggle the `opacity` of the
//! document's last object in a copy of the project: first `edits` of them
//! [`PACE`] apart, as a person makes them, each reloaded before its call
//! answers; then as many in a burst, which reload within the inspector's
//! budget and are caught up by the picture asked for after them. Then both
//! processes are left alone for a while to measure their idle CPU (rule 6:
//! 0 %).

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

const IDLE: Duration = Duration::from_secs(3);
/// Slower than the reload budget, so every paced edit is reloaded.
const PACE: Duration = Duration::from_millis(150);

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: preview_perf <project dir> <doc> [edits]";
    let source = args.next().expect(usage);
    let doc = args.next().expect(usage);
    let edits: usize = args.next().map_or(40, |n| n.parse().expect(usage));
    let copy = tempfile::tempdir().unwrap();
    copy_dir(Path::new(&source), copy.path());
    let state = tempfile::tempdir().unwrap();
    let state_dir = state.path().to_owned();
    let studio = Studio::open(copy.path())
        .unwrap()
        .with_previews(Arc::new(move |dir| {
            Box::new(NativeHost::new(
                dir,
                NativeHost::default_program(),
                &state_dir,
            ))
        }));
    let server = StudioServer::new(studio);
    let ctx = CallCtx::new(
        CapSet::default()
            .with(Cap::Inspect)
            .with(Cap::Mutate)
            .with(Cap::Run),
    );
    let call = |tool: &str, args: Value| -> Value {
        match server.call(tool, &ctx, args) {
            Ok(reply) => reply.structured,
            Err(e) => panic!("{tool}: {}", e.to_value()),
        }
    };

    let opened = call("preview_open", json!({ "doc": doc }));
    let tree = call("document_tree", json!({ "doc": doc, "limit": 100000 }));
    let last = tree["nodes"]
        .as_array()
        .and_then(|n| n.last())
        .cloned()
        .unwrap();
    let set = |i: usize| {
        let value = if i.is_multiple_of(2) { 0.5 } else { 1.0 };
        let started = Instant::now();
        call(
            "property_set",
            json!({ "doc": doc, "node": { "node": last["node"] }, "name": "opacity", "value": value }),
        );
        started.elapsed().as_secs_f64() * 1e3
    };
    let reloaded = |since: usize| -> (Vec<f64>, usize) {
        let events = call("preview_events", json!({ "since": since }));
        let events = events["events"].as_array().unwrap();
        let ms = events
            .iter()
            .filter(|e| e["kind"] == "reloaded")
            .filter_map(|e| e["ms"].as_f64())
            .collect();
        let failed = events
            .iter()
            .filter(|e| e["kind"] == "reload_failed")
            .count();
        (ms, failed)
    };

    let mut took = Vec::with_capacity(edits);
    for i in 0..edits {
        std::thread::sleep(PACE);
        took.push(set(i));
    }
    let (mut reloads, failed) = reloaded(0);
    let seen = call("preview_events", json!({}))["next"].as_u64().unwrap() as usize;

    let burst_began = Instant::now();
    let mut burst = Vec::with_capacity(edits);
    for i in 0..edits {
        burst.push(set(i + 1));
    }
    let burst_written = burst_began.elapsed().as_secs_f64() * 1e3;
    let caught_up = Instant::now();
    call("preview_screenshot", json!({ "format": "outline" }));
    let caught_up = caught_up.elapsed().as_secs_f64() * 1e3;
    let (burst_reloads, burst_failed) = reloaded(seen);

    let child = children()
        .into_iter()
        .next()
        .expect("the preview's process");
    let (self_before, child_before) = (cpu_ticks(std::process::id()), cpu_ticks(child));
    std::thread::sleep(IDLE);
    let (self_after, child_after) = (cpu_ticks(std::process::id()), cpu_ticks(child));
    let ticks = ticks_per_second();
    let percent = |a: u64, b: u64| (b - a) as f64 / ticks / IDLE.as_secs_f64() * 100.0;

    println!("preview of {doc} opened in {} ms", opened["ms"]);
    println!(
        "{edits} paced edits, written and shown: p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        percentile(&mut took, 50.0),
        percentile(&mut took, 95.0),
        took.iter().cloned().fold(0.0, f64::max)
    );
    println!(
        "{} reloads in the preview: p50 {:.1} ms, p95 {:.1} ms; {failed} failed",
        reloads.len(),
        percentile(&mut reloads, 50.0),
        percentile(&mut reloads, 95.0)
    );
    println!(
        "{edits} edits in a burst: written in {burst_written:.1} ms (max {:.1} ms each), \
         {} reloads, {burst_failed} failed; the outline after it in {caught_up:.1} ms",
        burst.iter().cloned().fold(0.0, f64::max),
        burst_reloads.len()
    );
    println!(
        "idle over {:?}: studio {:.1} % CPU, preview {:.1} % CPU, preview RSS {} MiB",
        IDLE,
        percent(self_before, self_after),
        percent(child_before, child_after),
        rss_kib(child) / 1024
    );
}

fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let rank = ((p / 100.0) * values.len() as f64).ceil() as usize;
    values[rank.clamp(1, values.len()) - 1]
}

/// This process's children, from every thread's list (Linux).
fn children() -> Vec<u32> {
    let mut out = Vec::new();
    for task in std::fs::read_dir("/proc/self/task").unwrap().flatten() {
        let text = std::fs::read_to_string(task.path().join("children")).unwrap_or_default();
        out.extend(
            text.split_whitespace()
                .filter_map(|p| p.parse::<u32>().ok()),
        );
    }
    out
}

/// User and system time a process has used, in clock ticks.
fn cpu_ticks(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    // The command name may hold spaces; the fields after it do not.
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap()
}

fn rss_kib(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn ticks_per_second() -> f64 {
    // Linux has used 100 since 2.6 on every architecture Ljós runs on.
    100.0
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            if entry.file_name() == "target" || entry.file_name() == ".git" {
                continue;
            }
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}
