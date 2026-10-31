//! The project's functional blocks, read through the session's store so that
//! a project in memory or in browser storage has them too.
//!
//! `ljos_blocks` reads a directory; Studio's project is a [`Store`], so this
//! walks the store the same way: type dumps first (a Rust block wins over a
//! QML one of the same name), then every `qmldir`'s `# block` comments and
//! their QML files.

use ljos_blocks::{BlockContract, BlockRegistry, read_qmldir};
use studio_core::{Store, StoreError};

/// The module that defines `Contribution` and `Slot` (ADR 0003, section 4).
pub(crate) const RUNTIME: &str = "Ljos.Blocks";

const DUMP: &str = "ljostypes.json";
const QMLDIR: &str = "qmldir";

/// Files named exactly `name`: [`Store::list`] matches a suffix.
fn named(store: &dyn Store, name: &str) -> Vec<String> {
    store
        .list(name)
        .into_iter()
        .filter(|p| p.rsplit('/').next() == Some(name))
        .collect()
}

/// `dir/file` with `.` and `..` resolved; `None` if it climbs out of the
/// project.
fn join(dir: &str, file: &str) -> Option<String> {
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

pub(crate) fn registry(store: &dyn Store) -> BlockRegistry {
    let mut registry = BlockRegistry::new();
    for file in named(store, DUMP) {
        match store.read(&file) {
            Ok(text) => {
                if let Err(e) = registry.add_type_dump(&text, &file) {
                    registry.problem(&file, None, e.to_string());
                }
            }
            Err(e) => registry.problem(&file, None, e.to_string()),
        }
    }
    for file in named(store, QMLDIR) {
        let text = match store.read(&file) {
            Ok(text) => text,
            Err(e) => {
                registry.problem(&file, None, e.to_string());
                continue;
            }
        };
        let read = read_qmldir(&text);
        for e in &read.errors {
            registry.problem(&file, Some(e.line), e.message.clone());
        }
        if read.blocks.is_empty() {
            continue;
        }
        let Some(module) = &read.module else {
            registry.problem(
                &file,
                None,
                "blocks of a qmldir without a `module` line belong to no module",
            );
            continue;
        };
        let dir = file.rsplit_once('/').map_or("", |(dir, _)| dir);
        for (line, decl) in &read.blocks {
            let line = Some(*line);
            let Some(qml) = join(dir, &decl.file) else {
                let message = format!("block {}: {} is outside the project", decl.name, decl.file);
                registry.problem(&file, line, message);
                continue;
            };
            match store.read(&qml) {
                Ok(source) => match ljos_blocks::qml_contract(module, decl, &qml, &source) {
                    Ok(contract) => registry.insert(contract, &file, line),
                    Err(message) => registry.problem(&file, line, message),
                },
                Err(StoreError::Missing(_)) => {
                    let message = format!("block {}: {qml} does not exist", decl.name);
                    registry.problem(&file, line, message);
                }
                Err(e) => registry.problem(&file, line, format!("block {}: {e}", decl.name)),
            }
        }
    }
    registry
}

/// The block `Module.Name` names: the module is everything before the last
/// dot.
pub(crate) fn find<'r>(registry: &'r BlockRegistry, qualified: &str) -> Option<&'r BlockContract> {
    let (module, name) = qualified.rsplit_once('.')?;
    registry.get(module, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_file_stays_inside_the_project() {
        assert_eq!(
            join("App/Media", "Player.qml").as_deref(),
            Some("App/Media/Player.qml")
        );
        assert_eq!(
            join("App/Media", "../Shared/A.qml").as_deref(),
            Some("App/Shared/A.qml")
        );
        assert_eq!(join("", "./A.qml").as_deref(), Some("A.qml"));
        assert_eq!(join("App", "../../A.qml"), None);
    }
}
