//! A JSON list's edit seen in the native preview (DESIGN.md, phase 3 / M1):
//! a `ListView` over a `JsonListModel` of 1,000 records, its file saved
//! with the first record changed, until the preview's outline shows the
//! change. The preview reads the file again itself (`ljos run --watch`);
//! nothing of the QML reloads. The outline is asked every 5 ms, inside the
//! preview's call rate, so a sample is late by at most that. Run it in a
//! release build:
//!
//! ```text
//! LJOS=…/ljos cargo run --release -p studio_preview --example json_edit [-- ROUNDS]
//! ```

use std::{
    fmt::Write as _,
    time::{Duration, Instant},
};

use studio_core::preview::{PreviewHost, PreviewSpec, Target};
use studio_preview::NativeHost;

const RECORDS: usize = 1000;
const POLL: Duration = Duration::from_millis(5);
const LIMIT: Duration = Duration::from_secs(10);
/// Between rounds, so one save's reading is over before the next.
const PACE: Duration = Duration::from_millis(250);

fn records(first: &str) -> String {
    let mut out = String::from("[");
    for i in 0..RECORDS {
        let name = if i == 0 {
            first.to_owned()
        } else {
            format!("Person {i}")
        };
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            r#"{{"name": "{name}", "team": "Team {}", "n": {i}}}"#,
            i % 7
        )
        .unwrap();
    }
    out.push(']');
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rounds: usize = match std::env::args().nth(1) {
        Some(n) => n.parse()?,
        None => 30,
    };
    let work = tempfile::tempdir()?;
    let project = work.path().join("project");
    let state = work.path().join("state");
    std::fs::create_dir_all(&project)?;
    std::fs::write(
        project.join("Main.qml"),
        "import QtQuick\nimport Ljos.Models\n\nListView {\n    width: 400\n    height: 600\n    \
         model: JsonListModel { source: \"people.json\" }\n    delegate: Text {\n        \
         Accessible.role: Accessible.StaticText\n        text: model.name + \", \" + model.team\n    \
         }\n}\n",
    )?;
    let data = project.join("people.json");
    std::fs::write(&data, records("Edit 0"))?;

    let mut host = NativeHost::new(&project, NativeHost::default_program(), &state);
    host.open(&PreviewSpec {
        entry: "Main.qml".into(),
        size: Target::Desktop.size(),
        aliases: Vec::new(),
        types: Vec::new(),
    })?;
    let shows =
        |host: &mut NativeHost, text: &str| -> Result<Duration, Box<dyn std::error::Error>> {
            let started = Instant::now();
            loop {
                if host.outline()?.contains(text) {
                    return Ok(started.elapsed());
                }
                if started.elapsed() > LIMIT {
                    return Err(format!("`{text}` not shown in {LIMIT:?}").into());
                }
                std::thread::sleep(POLL);
            }
        };
    shows(&mut host, "Edit 0, Team 0")?;

    let mut samples = Vec::with_capacity(rounds);
    for round in 1..=rounds {
        std::thread::sleep(PACE);
        let text = records(&format!("Edit {round}"));
        let saved = Instant::now();
        std::fs::write(&data, &text)?;
        shows(&mut host, &format!("Edit {round}, Team 0"))?;
        samples.push(saved.elapsed());
    }
    samples.sort();
    let at = |q: f64| samples[((samples.len() as f64 - 1.0) * q).round() as usize];
    println!(
        "JSON of {RECORDS} records saved -> the preview shows the edit, {rounds} rounds \
         ({} bytes): p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        records("Edit 0").len(),
        at(0.5).as_secs_f64() * 1e3,
        at(0.95).as_secs_f64() * 1e3,
        samples.last().unwrap().as_secs_f64() * 1e3,
    );
    host.close();
    Ok(())
}
