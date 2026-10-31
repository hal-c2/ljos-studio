//! Times `export` as an agent calls it on a project whose bundle holds 200
//! files: a shell, 160 documents of 40 objects each, 20 blocks in four
//! modules with their qmldirs, a theme and assets, beside four mock
//! modules that are left out. Every export after the first
//! replaces the last bundle, as re-exporting after an edit does. `web`
//! writes the site bundle instead, whose imports are checked too.
//!
//! `dry` as a third argument times the dry run the window's export panel
//! asks for on opening instead.
//!
//! `cargo run --release -p studio_mcp --example export_perf [-- EXPORTS [native|web [dry]]]`

use std::{
    path::Path,
    time::{Duration, Instant},
};

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::json;
use studio_mcp::{Studio, StudioServer};

const DOCUMENTS: usize = 160;
const OBJECTS: usize = 40;
const MODULES: usize = 4;
const BLOCKS: usize = 5;
const ASSETS: usize = 14;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn main() {
    let exports: usize = std::env::args()
        .nth(1)
        .map_or(20, |n| n.parse().expect("EXPORTS is a number"));
    let target = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "native".to_owned());
    let dry = std::env::args().nth(3).is_some_and(|a| a == "dry");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut files = 0;
    for m in 0..MODULES {
        let mut qmldir = format!("module App.M{m}\n");
        let mut mock = qmldir.clone();
        for b in 0..BLOCKS {
            qmldir += &format!("# block B{b} 1.0 B{b}.qml App \"Block {b}\"\n");
            mock += &format!("B{b} 1.0 B{b}.qml\n");
            write(
                root,
                &format!("App/M{m}/B{b}.qml"),
                "import QtQuick\n\nItem {\n    property string label\n}\n",
            );
            files += 1;
        }
        write(root, &format!("App/M{m}/qmldir"), &qmldir);
        // The mocks are left out of the bundle: walked, not copied.
        write(root, &format!("mocks/App/M{m}/qmldir"), &mock);
        files += 1;
    }
    for d in 0..DOCUMENTS {
        let mut text = format!("import QtQuick\nimport App.M{}\n\nColumn {{\n", d % MODULES);
        for i in 0..OBJECTS {
            text +=
                &format!("    Rectangle {{ id: r{i}; width: {i}; height: 10; color: \"grey\" }}\n");
        }
        text += "}\n";
        write(root, &format!("screens/S{d}.qml"), &text);
        files += 1;
    }
    write(root, "theme.json", "{\"colors\": {}}\n");
    files += 1;
    for a in 0..ASSETS {
        write(
            root,
            &format!("images/a{a}.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"/>\n",
        );
        files += 1;
    }
    write(root, "Shell.qml", "import QtQuick\n\nItem {}\n");
    files += 1;
    assert_eq!(files, 200, "the bundle is to hold 200 files");

    let server = StudioServer::new(Studio::open(root).unwrap());
    let ctx = CallCtx::new(CapSet::default().with(Cap::Inspect).with(Cap::Fs));
    let mut samples: Vec<Duration> = Vec::new();
    let mut bytes = 0;
    let mut manifest = 0;
    for _ in 0..exports {
        let started = Instant::now();
        let (out, _) = server.tools().call(
            "export",
            &ctx,
            json!({ "entry": "Shell.qml", "target": target, "dry_run": dry }),
        );
        samples.push(started.elapsed());
        let out = out.unwrap_or_else(|e| panic!("export was refused: {}", e.to_value()));
        assert_eq!(out["files"], 200);
        bytes = out["bytes"].as_u64().unwrap();
        if dry {
            continue;
        }
        let name = if target == "web" {
            studio_mcp::export::SITE_MANIFEST
        } else {
            studio_mcp::export::MANIFEST
        };
        manifest = std::fs::metadata(root.join(out["out"].as_str().unwrap()).join(name))
            .unwrap()
            .len();
    }
    samples.sort();
    let at = |p: f64| samples[((samples.len() - 1) as f64 * p).round() as usize];
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "{target} {} of 200 files ({bytes} bytes and a manifest of {manifest}): p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms over {}",
        if dry { "dry run" } else { "export" },
        ms(at(0.5)),
        ms(at(0.95)),
        ms(at(1.0)),
        samples.len()
    );
}
