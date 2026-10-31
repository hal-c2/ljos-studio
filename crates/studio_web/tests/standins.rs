//! ljos-web's compiler knows Studio's native types from QML stand-ins
//! (`web/standins/Studio`) until it reads `ljostypes.json`; a stand-in is a
//! second statement of a type and can drift from it. This holds the two to
//! one: every member the dump has, the stand-in declares as the dump says.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The repository, from the crate's directory as cargo names it to the
/// running test: the shared compile cache may hand a binary built in
/// another clone, whose `env!` path is that clone's.
fn root() -> PathBuf {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    Path::new(&crate_dir).join("../..")
}

#[test]
fn the_stand_ins_say_what_the_type_dump_says() {
    let dump: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("crates/studio_blocks/ljostypes.json")).unwrap(),
    )
    .unwrap();
    let dir = root().join("web/standins/Studio");
    let qmldir = std::fs::read_to_string(dir.join("qmldir")).unwrap();
    let mut missing = Vec::new();
    for module in dump["modules"].as_array().unwrap() {
        assert_eq!(module["uri"], "Studio");
        for ty in module["types"].as_array().unwrap() {
            let name = ty["name"].as_str().unwrap();
            if !qmldir.contains(&format!("{name} 1.0 {name}.qml")) {
                missing.push(format!("qmldir: {name}"));
            }
            let text = std::fs::read_to_string(dir.join(format!("{name}.qml"))).unwrap();
            let base = ty["base"].as_str().unwrap();
            if !text.contains(&format!("\n{base} {{")) {
                missing.push(format!("{name}: based on {base}"));
            }
            for p in ty["properties"].as_array().unwrap() {
                let readonly = if p["readonly"] == true {
                    "readonly "
                } else {
                    ""
                };
                let line = format!(
                    "{readonly}property {} {}",
                    p["type"].as_str().unwrap(),
                    p["name"].as_str().unwrap()
                );
                if !text.contains(&line) {
                    missing.push(format!("{name}: {line}"));
                }
            }
            for s in ty["signals"].as_array().unwrap() {
                let line = format!("signal {}({})", s["name"].as_str().unwrap(), parameters(s));
                if !text.contains(&line) {
                    missing.push(format!("{name}: {line}"));
                }
            }
            for m in ty["methods"].as_array().unwrap() {
                let params: Vec<String> = m["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        format!(
                            "{}: {}",
                            p["name"].as_str().unwrap(),
                            p["type"].as_str().unwrap()
                        )
                    })
                    .collect();
                let line = format!(
                    "function {}({})",
                    m["name"].as_str().unwrap(),
                    params.join(", ")
                );
                if !text.contains(&line) {
                    missing.push(format!("{name}: {line}"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "the stand-ins lack:\n{}",
        missing.join("\n")
    );
}

fn parameters(signal: &Value) -> String {
    signal["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            format!(
                "{} {}",
                p["type"].as_str().unwrap(),
                p["name"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
