//! A site builder held to the contract a web export promises (DESIGN.md
//! S6, the site contract): the stand-in for ljos-web's, for tests only.
//!
//! It reads `ljos-site.json` by the contract's words, not by Studio's own
//! types, and it builds nothing it was not promised: a field it does not
//! know, a file the manifest does not list or lists wrong, an import that
//! neither the project nor ljos-web provides, anything of Studio's. What it
//! "builds" is the list of documents a site would compile, each with what
//! it was resolved against, so a test can see the site starts where the
//! manifest says.

// Each test binary that includes it uses a part of it.
#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use serde::Deserialize;

const MANIFEST: &str = "ljos-site.json";

/// What this builder provides of ljos-web: fewer modules than ljos-web
/// does, which makes it stricter, never laxer.
const PROVIDED: &[&str] = &[
    "Ljos.Design",
    "QtQml",
    "QtQuick",
    "QtQuick.Controls",
    "QtQuick.Layouts",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    version: u32,
    target: String,
    entry: String,
    #[serde(default)]
    theme: Option<String>,
    blocks: Vec<Block>,
    files: Vec<File>,
    left_out: Vec<LeftOut>,
    modules: Vec<Module>,
    imports: Vec<String>,
    ljos_web: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Block {
    block: String,
    file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
    bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct LeftOut {
    path: String,
    why: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Module {
    module: String,
    qmldir: String,
}

/// What a site would be built from.
#[derive(Debug)]
pub struct Site {
    pub entry: String,
    /// Each document the site compiles and the modules it resolved.
    pub documents: BTreeMap<String, Vec<String>>,
    pub theme: Option<String>,
}

/// Builds the site bundle in `dir`, which must start at `entry`.
pub fn build(dir: &Path, entry: &str) -> Result<Site, String> {
    let text = fs::read_to_string(dir.join(MANIFEST)).map_err(|e| format!("no {MANIFEST}: {e}"))?;
    let m: Manifest = serde_json::from_str(&text)
        .map_err(|e| format!("{MANIFEST} is not the contract's: {e}"))?;
    if (m.format.as_str(), m.version, m.target.as_str()) != ("ljos-site", 1, "web") {
        return Err(format!(
            "a {} {} for `{}` is not a site bundle this builder reads",
            m.format, m.version, m.target
        ));
    }
    if m.ljos_web.len() != 40 || !m.ljos_web.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("`{}` is not an ljos-web commit", m.ljos_web));
    }
    if m.entry != entry {
        return Err(format!("the site starts at `{}`, not `{entry}`", m.entry));
    }

    let mut listed = BTreeSet::new();
    for f in &m.files {
        check_path(&f.path)?;
        if !listed.insert(f.path.as_str()) {
            return Err(format!("`{}` is listed twice", f.path));
        }
        let meta = fs::symlink_metadata(dir.join(&f.path))
            .map_err(|e| format!("`{}` is listed but not there: {e}", f.path))?;
        if !meta.file_type().is_file() {
            return Err(format!("`{}` is not a plain file", f.path));
        }
        if meta.len() != f.bytes {
            return Err(format!(
                "`{}` is {} bytes, the manifest says {}",
                f.path,
                meta.len(),
                f.bytes
            ));
        }
    }
    let mut present = Vec::new();
    files_below(dir, "", &mut present)?;
    for path in &present {
        if path != MANIFEST && !listed.contains(path.as_str()) {
            return Err(format!("`{path}` is in the bundle but not in its manifest"));
        }
    }
    if !entry.ends_with(".qml") || !listed.contains(entry) {
        return Err(format!("the entry `{entry}` is not a listed document"));
    }
    if let Some(theme) = &m.theme
        && !listed.contains(theme.as_str())
    {
        return Err(format!("the theme `{theme}` is not a listed file"));
    }

    let mut own = BTreeSet::new();
    for module in &m.modules {
        if !listed.contains(module.qmldir.as_str()) || !module.qmldir.ends_with("qmldir") {
            return Err(format!(
                "`{}`'s qmldir `{}` is not a listed qmldir",
                module.module, module.qmldir
            ));
        }
        let qmldir = fs::read_to_string(dir.join(&module.qmldir)).map_err(|e| e.to_string())?;
        let declared = qmldir.lines().find_map(|l| l.strip_prefix("module "));
        if declared.map(str::trim) != Some(module.module.as_str()) {
            return Err(format!(
                "`{}` declares {declared:?}, the manifest says `{}`",
                module.qmldir, module.module
            ));
        }
        let base = module.qmldir.rsplit_once('/').map_or("", |(d, _)| d);
        for file in qmldir
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_whitespace().nth(2))
            .filter(|f| f.ends_with(".qml") || f.ends_with(".js"))
        {
            let path = if base.is_empty() {
                file.to_owned()
            } else {
                format!("{base}/{file}")
            };
            if !listed.contains(path.as_str()) {
                return Err(format!(
                    "`{}` names `{path}`, which is not listed",
                    module.qmldir
                ));
            }
        }
        if !own.insert(module.module.as_str()) {
            return Err(format!(
                "`{}` is in `modules` twice: two directories would claim it",
                module.module
            ));
        }
    }
    for import in &m.imports {
        if !PROVIDED.contains(&import.as_str()) {
            return Err(format!("ljos-web provides no `{import}` to this builder"));
        }
    }

    let mut documents = BTreeMap::new();
    for path in listed.iter().filter(|p| p.ends_with(".qml")) {
        let text = fs::read_to_string(dir.join(path)).map_err(|e| e.to_string())?;
        let imports = studio_core::outline::imports(&text)
            .ok_or_else(|| format!("`{path}` does not parse"))?;
        let mut resolved = Vec::new();
        for import in imports {
            match import {
                studio_core::outline::Import::Module { uri: name, .. } => {
                    if !own.contains(name.as_str()) && !m.imports.contains(&name) {
                        return Err(format!(
                            "`{path}` imports `{name}`, which the manifest neither lists nor imports"
                        ));
                    }
                    resolved.push(name);
                }
                studio_core::outline::Import::Path { path: p, .. } => resolved.push(p),
            }
        }
        documents.insert((*path).to_owned(), resolved);
    }
    // The real builder finds modules by the qmldirs below the site, so a
    // listed qmldir that declares a module `modules` does not name is a
    // module the manifest hides.
    let named: BTreeSet<&str> = m.modules.iter().map(|d| d.qmldir.as_str()).collect();
    for qmldir in listed
        .iter()
        .filter(|p| p.rsplit('/').next() == Some("qmldir") && !named.contains(*p))
    {
        let text = fs::read_to_string(dir.join(qmldir)).map_err(|e| e.to_string())?;
        if let Some(module) = text.lines().find_map(|l| l.strip_prefix("module ")) {
            return Err(format!(
                "`{qmldir}` declares module {} but `modules` does not name it",
                module.trim()
            ));
        }
    }
    Ok(Site {
        entry: m.entry,
        documents,
        theme: m.theme,
    })
}

/// A path the contract allows: plain names below the bundle, none of them
/// Studio's.
fn check_path(path: &str) -> Result<(), String> {
    let plain = Path::new(path)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if !plain || path.is_empty() || path.contains('\\') {
        return Err(format!("`{path}` is not a plain path below the bundle"));
    }
    let first = path.split('/').next().unwrap_or(path);
    if first == "mocks" || path.split('/').any(|p| p == ".studio") {
        return Err(format!("`{path}` is Studio's, not the app's"));
    }
    if path.rsplit('/').next() == Some("ljostypes.json") {
        return Err(format!("`{path}` declares types written in Rust"));
    }
    Ok(())
}

fn files_below(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        if ty.is_symlink() {
            return Err(format!("`{path}` is a link"));
        } else if ty.is_dir() {
            files_below(&entry.path(), &format!("{path}/"), out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
