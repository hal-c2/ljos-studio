//! The site builder held to the contract (`tests/site/mod.rs`) builds what
//! a web export writes and refuses every bundle the contract does not
//! promise, so a scenario that passes through it is evidence about the
//! contract, not about a builder that takes anything.

mod site;

use std::{fs, path::Path};

use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::{Value, json};
use studio_mcp::{Studio, StudioServer};

/// A project exported for the web, at `<dir>/build/web`.
fn exported() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let write = |path: &str, text: &str| {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    };
    write(
        "App/Media/qmldir",
        "module App.Media\nPlayer 1.0 Player.qml\n",
    );
    write("App/Media/Player.qml", "import QtQuick\nItem {}\n");
    write(
        "Shell.qml",
        "import QtQuick\nimport App.Media\nimport Ljos.Design\nColumn { Player {} Text { text: \"Hi\" } }\n",
    );
    write("theme.json", "{}\n");
    let studio = Studio::open(dir.path()).unwrap();
    let server = StudioServer::new(studio);
    server
        .call(
            "export",
            &CallCtx::new(CapSet::of([Cap::Inspect, Cap::Fs])),
            json!({"entry": "Shell.qml", "target": "web"}),
        )
        .unwrap();
    dir
}

fn bundle(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("build/web")
}

fn manifest(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("ljos-site.json")).unwrap()).unwrap()
}

fn set_manifest(dir: &Path, value: &Value) {
    fs::write(dir.join("ljos-site.json"), value.to_string()).unwrap();
}

/// Builds after `tamper` and expects a refusal that mentions `says`.
fn refused(tamper: impl FnOnce(&Path), says: &str) {
    let dir = exported();
    let out = bundle(&dir);
    tamper(&out);
    match site::build(&out, "Shell.qml") {
        Ok(site) => panic!("built {site:?}, expected a refusal saying `{says}`"),
        Err(e) => assert!(e.contains(says), "refused with `{e}`, not `{says}`"),
    }
}

#[test]
fn an_export_builds() {
    let dir = exported();
    let site = site::build(&bundle(&dir), "Shell.qml").unwrap();
    assert_eq!(site.entry, "Shell.qml");
    assert_eq!(
        site.documents["Shell.qml"],
        ["QtQuick", "App.Media", "Ljos.Design"]
    );
    assert_eq!(site.theme.as_deref(), Some("theme.json"));
}

#[test]
fn another_entry_is_not_this_site() {
    let dir = exported();
    let e = site::build(&bundle(&dir), "App/Media/Player.qml").unwrap_err();
    assert!(e.contains("starts at"), "{e}");
}

#[test]
fn a_file_the_manifest_does_not_list() {
    refused(
        |d| fs::write(d.join("extra.qml"), "Item {}\n").unwrap(),
        "not in its manifest",
    );
}

#[test]
fn a_listed_file_that_is_missing() {
    refused(
        |d| fs::remove_file(d.join("theme.json")).unwrap(),
        "not there",
    );
}

#[test]
fn a_file_of_other_bytes() {
    refused(
        |d| fs::write(d.join("theme.json"), "{ }\n").unwrap(),
        "the manifest says",
    );
}

#[test]
fn a_path_out_of_the_bundle() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["files"][0]["path"] = json!("../Shell.qml");
            set_manifest(d, &m);
        },
        "not a plain path",
    );
}

#[test]
fn an_import_ljos_web_does_not_provide() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["imports"]
                .as_array_mut()
                .unwrap()
                .push(json!("Ljos.Blocks"));
            set_manifest(d, &m);
        },
        "provides no `Ljos.Blocks`",
    );
}

#[test]
fn an_import_the_manifest_does_not_say() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["imports"] = json!(["QtQuick"]);
            set_manifest(d, &m);
        },
        "imports `Ljos.Design`",
    );
}

#[test]
fn a_module_without_its_qmldir() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["modules"] = json!([]);
            set_manifest(d, &m);
        },
        "imports `App.Media`",
    );
}

#[test]
fn a_module_named_twice() {
    refused(
        |d| {
            let mut m = manifest(d);
            let again = m["modules"][0].clone();
            m["modules"].as_array_mut().unwrap().push(again);
            set_manifest(d, &m);
        },
        "twice",
    );
}

#[test]
fn a_listed_qmldir_whose_module_the_manifest_hides() {
    refused(
        |d| {
            let text = "module App.Media\nPlayer 1.0 Player.qml\n";
            fs::create_dir_all(d.join("media")).unwrap();
            fs::write(d.join("media/qmldir"), text).unwrap();
            fs::copy(d.join("App/Media/Player.qml"), d.join("media/Player.qml")).unwrap();
            let mut m = manifest(d);
            let files = m["files"].as_array_mut().unwrap();
            for (path, bytes) in [
                ("media/qmldir", text.len() as u64),
                (
                    "media/Player.qml",
                    fs::metadata(d.join("media/Player.qml")).unwrap().len(),
                ),
            ] {
                files.push(json!({"path": path, "bytes": bytes}));
            }
            set_manifest(d, &m);
        },
        "does not name it",
    );
}

#[test]
fn a_module_whose_qmldir_says_another() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["modules"][0]["module"] = json!("App.Other");
            set_manifest(d, &m);
        },
        "declares",
    );
}

#[test]
fn a_component_its_qmldir_names_that_is_not_listed() {
    refused(
        |d| {
            fs::remove_file(d.join("App/Media/Player.qml")).unwrap();
            let mut m = manifest(d);
            m["files"]
                .as_array_mut()
                .unwrap()
                .retain(|f| f["path"] != "App/Media/Player.qml");
            set_manifest(d, &m);
        },
        "which is not listed",
    );
}

#[test]
fn another_version() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["version"] = json!(2);
            set_manifest(d, &m);
        },
        "not a site bundle",
    );
}

#[test]
fn a_native_bundle() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["target"] = json!("native");
            set_manifest(d, &m);
        },
        "not a site bundle",
    );
}

#[test]
fn a_field_the_contract_does_not_have() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["run"] = json!(["ljos", "run", "Shell.qml"]);
            set_manifest(d, &m);
        },
        "not the contract's",
    );
}

#[test]
fn an_entry_that_is_not_a_document() {
    let dir = exported();
    let out = bundle(&dir);
    let mut m = manifest(&out);
    m["entry"] = json!("theme.json");
    set_manifest(&out, &m);
    let e = site::build(&out, "theme.json").unwrap_err();
    assert!(e.contains("not a listed document"), "{e}");
}

#[test]
fn mocks_and_the_sidecar_are_studios() {
    for path in ["mocks/App/Media/qmldir", ".studio/review.json"] {
        refused(
            |d| {
                let full = d.join(path);
                fs::create_dir_all(full.parent().unwrap()).unwrap();
                fs::write(&full, "x").unwrap();
                let mut m = manifest(d);
                m["files"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"path": path, "bytes": 1}));
                set_manifest(d, &m);
            },
            "is Studio's",
        );
    }
}

#[test]
fn types_written_in_rust() {
    refused(
        |d| {
            fs::write(d.join("ljostypes.json"), "{}").unwrap();
            let mut m = manifest(d);
            m["files"]
                .as_array_mut()
                .unwrap()
                .push(json!({"path": "ljostypes.json", "bytes": 2}));
            set_manifest(d, &m);
        },
        "written in Rust",
    );
}

#[test]
fn an_ljos_web_that_is_no_commit() {
    refused(
        |d| {
            let mut m = manifest(d);
            m["ljos_web"] = json!("main");
            set_manifest(d, &m);
        },
        "not an ljos-web commit",
    );
}

#[cfg(unix)]
#[test]
fn a_link() {
    refused(
        |d| {
            fs::remove_file(d.join("theme.json")).unwrap();
            std::os::unix::fs::symlink("/etc/hostname", d.join("theme.json")).unwrap();
        },
        "not a plain file",
    );
}

/// Studio's list of what ljos-web provides is the runtime package's exports
/// at the pin, whatever the checkout has out, when one is at hand (`LJOS_WEB`).
/// `mise run check:web-modules` always has one, and CI runs it.
#[test]
fn studio_knows_the_modules_ljos_web_exports() {
    let Ok(web) = std::env::var("LJOS_WEB") else {
        eprintln!("LJOS_WEB is not set: the list is not compared with a checkout");
        return;
    };
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let pin = fs::read_to_string(Path::new(&crate_dir).join("../../web/ljos-web.rev")).unwrap();
    let shown = std::process::Command::new("git")
        .args(["-C", &web, "show"])
        .arg(format!("{}:packages/runtime/package.json", pin.trim()))
        .output()
        .unwrap();
    assert!(
        shown.status.success(),
        "{web} does not have the pin {}: {}",
        pin.trim(),
        String::from_utf8_lossy(&shown.stderr)
    );
    let package: Value = serde_json::from_slice(&shown.stdout).unwrap();
    let mut exported: Vec<String> = package["exports"]
        .as_object()
        .unwrap()
        .keys()
        .filter_map(|k| k.strip_prefix("./"))
        .filter(|k| !matches!(*k, "runtime" | "runtime.css" | "object" | "vite"))
        .map(|k| k.replace('/', "."))
        .collect();
    // Compiled from Qt's own QML and from Ljós's, not shipped as modules.
    exported.extend(["QtQuick.Controls".to_owned(), "Ljos.Design".to_owned()]);
    exported.sort();
    let mut studio: Vec<&str> = studio_mcp::export::WEB_MODULES.to_vec();
    studio.sort();
    assert_eq!(studio, exported);
}
