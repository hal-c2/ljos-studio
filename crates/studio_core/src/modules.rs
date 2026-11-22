//! What the project's own modules export, and whether a name in a document
//! may be one of the app's: Studio adds an import only for a name that is
//! nobody else's.

use crate::{Import, Store, outline};

/// The project's modules, each with the names its `qmldir`s and type dumps
/// export.
pub struct Modules(Vec<(String, Vec<String>)>);

impl Modules {
    /// Mock modules are the preview's, not the app's.
    pub fn read(store: &dyn Store) -> Self {
        let files = |name: &'static str| {
            store.list(name).into_iter().filter(move |path| {
                !path.starts_with("mocks/") && path.rsplit('/').next() == Some(name)
            })
        };
        let mut modules = Vec::new();
        for path in files("qmldir") {
            let Ok(text) = store.read(&path) else {
                continue;
            };
            let uri = text.lines().find_map(|line| {
                let mut words = line.split_whitespace();
                (words.next() == Some("module")).then(|| words.next())?
            });
            if let Some(uri) = uri {
                let names = exported(&text).map(str::to_owned).collect();
                modules.push((uri.to_owned(), names));
            }
        }
        for path in files("ljostypes.json") {
            let Some(dump) = store
                .read(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            else {
                continue;
            };
            for module in dump["modules"].as_array().into_iter().flatten() {
                if let Some(uri) = module["uri"].as_str() {
                    let names = module["types"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|ty| ty["name"].as_str().map(str::to_owned))
                        .collect();
                    modules.push((uri.to_owned(), names));
                }
            }
        }
        Self(modules)
    }

    /// The modules that export `name`, each once.
    pub fn exporters(&self, name: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for (uri, names) in &self.0 {
            if !out.contains(&uri.as_str()) && names.iter().any(|n| n == name) {
                out.push(uri);
            }
        }
        out
    }

    fn describes(&self, uri: &str) -> bool {
        self.0.iter().any(|(module, _)| module == uri)
    }
}

/// The names a `qmldir` exports: its types, singletons and announced blocks.
/// An `internal` type is the module's own.
fn exported(qmldir: &str) -> impl Iterator<Item = &str> {
    qmldir.lines().filter_map(|line| {
        let line = line.trim_start();
        let line = line.strip_prefix("# block ").unwrap_or(line);
        let code = line.split('#').next().unwrap_or(line);
        let mut words = code.split_whitespace();
        let name = match words.next()? {
            "singleton" => words.next()?,
            first => first,
        };
        name.starts_with(|c: char| c.is_ascii_uppercase())
            .then_some(name)
    })
}

/// `dir/file` with `.` and `..` resolved; `None` if it climbs out of the
/// project.
pub fn join(dir: &str, file: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in file.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// Each capitalised name `text` reads a member of (`Colors.accent`,
/// `Layout.fillWidth`), once for each time, leaving out strings and comments.
pub fn member_uses(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = text[i + 2..].find("*/").map_or(bytes.len(), |n| i + n + 4);
            }
            quote @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b if word(b) => {
                let start = i;
                while i < bytes.len() && word(bytes[i]) {
                    i += 1;
                }
                let after_dot = start > 0 && bytes[start - 1] == b'.';
                if !after_dot && b.is_ascii_uppercase() && bytes.get(i) == Some(&b'.') {
                    out.push(&text[start..i]);
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Whether `name`, in the document `doc` whose text is `text`, is or may be
/// something other than `module`'s: the document's own, a file's beside it or
/// in a folder it imports, or a module's of the app. What can't be read may
/// have it.
pub(crate) fn may_be_anothers(
    store: &dyn Store,
    modules: &Modules,
    doc: &str,
    text: &str,
    name: &str,
    module: &str,
) -> bool {
    let (Some(imports), Some(nodes)) = (outline::imports(text), outline::outline(text)) else {
        return true;
    };
    if outline::declares_component(&nodes, name)
        || modules.exporters(name).iter().any(|uri| *uri != module)
    {
        return true;
    }
    let here = doc.rsplit_once('/').map_or("", |(dir, _)| dir);
    let folder_has = |dir: &str| {
        let file = |file: &str| match dir {
            "" => file.to_owned(),
            _ => format!("{dir}/{file}"),
        };
        store.exists(&file(&format!("{name}.qml")))
            || store
                .read(&file("qmldir"))
                .is_ok_and(|listing| exported(&listing).any(|n| n == name))
    };
    folder_has(here)
        || imports.iter().any(|import| match import {
            Import::Module { alias: Some(a), .. } | Import::Path { alias: Some(a), .. } => {
                a == name
            }
            Import::Path { path, alias: None } => {
                if path.starts_with('/') || path.contains(':') {
                    return true;
                }
                join(here, path).is_none_or(|dir| folder_has(&dir))
            }
            // The toolkit's modules and Ljós's have none of these names.
            Import::Module { uri, alias: None } => {
                !uri.starts_with("Qt") && !uri.starts_with("Ljos.") && !modules.describes(uri)
            }
        })
}
