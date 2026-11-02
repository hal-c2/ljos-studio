//! Where a name lands, given a file's imports in order. One resolver for the
//! file a thing came from and the file it goes to, so a caller compares answers
//! and never reasons about import order. `measured_lookup.txt` is what qml6 said,
//! and the test below holds this to it.

use serde::Serialize;

use super::{Described, Found, Import, Names, measured_types};

/// Where a name lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Lands {
    /// A file in this folder of the project.
    Folder(String),
    /// A type these modules have.
    Modules(Vec<String>),
    /// qml6 would not find it.
    Nowhere,
    /// Something Studio can't read might change the answer.
    CantTell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Keeps {
    Same,
    Different,
    CantTell,
}

/// Whether a name that lands at `was` is the same thing landing at `now`.
pub(crate) fn compare(was: &Lands, now: &Lands) -> Keeps {
    match (was, now) {
        (Lands::CantTell, _) | (_, Lands::CantTell) | (Lands::Nowhere, Lands::Nowhere) => {
            Keeps::CantTell
        }
        (Lands::Folder(a), Lands::Folder(b)) if a == b => Keeps::Same,
        (Lands::Modules(a), Lands::Modules(b)) if a.iter().any(|m| b.contains(m)) => Keeps::Same,
        _ => Keeps::Different,
    }
}

impl Lands {
    /// For a person reading what a write couldn't vouch for.
    pub(crate) fn say(&self) -> String {
        match self {
            Lands::Folder(dir) if dir.is_empty() => "the project's root folder".to_owned(),
            Lands::Folder(dir) => format!("the folder {dir}"),
            Lands::Modules(modules) => modules.join(", "),
            Lands::Nowhere => "nothing".to_owned(),
            Lands::CantTell => "can't tell".to_owned(),
        }
    }
}

impl Names<'_> {
    /// Where qml6 finds `name` with these imports: the folder imported last,
    /// before any module, before the file beside the document. `Head.Inner` is
    /// in what its alias names, or else in the file `Head` lands on.
    pub(crate) fn lands(&self, name: &str) -> Lands {
        let (head, inner) = match name.split_once('.') {
            Some((head, inner)) => (head, Some(inner)),
            None => (name, None),
        };
        if let Some(inner) = inner
            && let Some(import) = self.scope.imports.iter().find(|import| {
                matches!(import, Import::Module { alias: Some(a), .. }
                    | Import::Path { alias: Some(a), .. } if a == head)
            })
        {
            return self.in_alias(import, inner);
        }
        for import in self.scope.imports.iter().rev() {
            let Import::Path { path, alias: None } = import else {
                continue;
            };
            if path.ends_with(".js") {
                continue;
            }
            if self.unread(import) {
                return Lands::CantTell;
            }
            if let Some(dir) = crate::blocks::join(&self.scope.dir, path)
                && self.folder_has(&dir, head)
            {
                return Lands::Folder(dir);
            }
        }
        let bare_modules = || {
            self.scope.imports.iter().filter_map(|import| match import {
                Import::Module { uri, alias: None } => Some((import, uri)),
                _ => None,
            })
        };
        let modules: Vec<String> = bare_modules()
            .filter(|(_, uri)| self.module_has(uri, head))
            .map(|(_, uri)| uri.clone())
            .collect();
        if !modules.is_empty() {
            // A module's type holds no inner one.
            return match inner {
                Some(_) => Lands::Nowhere,
                None => Lands::Modules(modules),
            };
        }
        if bare_modules().any(|(import, _)| self.unread(import)) {
            return Lands::CantTell;
        }
        if self.folder_has(&self.scope.dir, head) {
            return Lands::Folder(self.scope.dir.clone());
        }
        let untried = inner.is_none()
            && !measured_types().contains_key(head)
            && bare_modules().next().is_some();
        if untried {
            Lands::CantTell
        } else {
            Lands::Nowhere
        }
    }

    fn in_alias(&self, import: &Import, inner: &str) -> Lands {
        let head = inner.split('.').next().unwrap_or(inner);
        match import {
            Import::Path { path, .. } => match crate::blocks::join(&self.scope.dir, path) {
                Some(dir) if !self.unread(import) => match self.folder_has(&dir, head) {
                    true => Lands::Folder(dir),
                    false => Lands::Nowhere,
                },
                _ => Lands::CantTell,
            },
            Import::Module { uri, .. } => {
                if self.unread(import) || self.module_has(uri, head) {
                    Lands::Modules(vec![uri.clone()])
                } else if measured_types().contains_key(head) {
                    Lands::Nowhere
                } else {
                    Lands::CantTell
                }
            }
        }
    }

    fn folder_has(&self, dir: &str, name: &str) -> bool {
        self.types.in_dir(dir, name, &mut Found::default(), 0) != Described::Not
    }

    fn module_has(&self, uri: &str, name: &str) -> bool {
        measured_types()
            .get(name)
            .is_some_and(|modules| modules.contains(uri))
            || self.types.in_module(uri, name, &mut Found::default(), 0) != Described::Not
    }
}

/// A name a write could not show still lands where it did.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Unvouched {
    #[serde(rename = "type")]
    pub ty: String,
    /// Where it was found in the file it came from.
    pub was: String,
    /// Where it is found now.
    pub now: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_core::MemStore;

    fn import(spec: &str) -> Import {
        let (what, alias) = match spec.split_once(" as ") {
            Some((what, alias)) => (what, Some(alias.to_owned())),
            None => (spec, None),
        };
        if what.contains('/') || what == "." {
            Import::Path {
                path: what.to_owned(),
                alias,
            }
        } else {
            Import::Module {
                uri: what.to_owned(),
                alias,
            }
        }
    }

    fn answer(lands: &Lands) -> String {
        match lands {
            Lands::Folder(dir) if dir.is_empty() => ".".to_owned(),
            Lands::Folder(dir) => dir.clone(),
            Lands::Modules(_) => "module".to_owned(),
            Lands::Nowhere => "none".to_owned(),
            Lands::CantTell => "can't tell".to_owned(),
        }
    }

    #[test]
    fn the_resolver_answers_what_qml6_answered() {
        let mut wrong = Vec::new();
        for line in include_str!("../measured_lookup.txt").lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let [case, document, imports, files, name, measured] = cells[..] else {
                panic!("a row has six cells: {line}");
            };
            let store = MemStore::new(files.split_whitespace().map(|file| {
                (
                    format!("{file}.qml"),
                    "import QtQuick\nItem {}\n".to_owned(),
                )
            }));
            let imports: Vec<Import> = imports
                .split(';')
                .map(str::trim)
                .filter(|spec| !spec.is_empty())
                .map(import)
                .collect();
            let got = answer(&Names::at(&store, document, &imports).lands(name));
            if got != measured {
                wrong.push(format!("{case}: qml6 says {measured}, the resolver {got}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn what_compares_as_the_same() {
        let folder = |dir: &str| Lands::Folder(dir.to_owned());
        let modules = |uri: &str| Lands::Modules(vec![uri.to_owned()]);
        let rows = [
            (folder("a"), folder("a"), Keeps::Same),
            (folder("a"), folder("b"), Keeps::Different),
            (folder("a"), modules("M"), Keeps::Different),
            (folder("a"), Lands::Nowhere, Keeps::Different),
            (modules("M"), modules("M"), Keeps::Same),
            (modules("M"), modules("N"), Keeps::Different),
            (Lands::Nowhere, Lands::Nowhere, Keeps::CantTell),
            (Lands::CantTell, folder("a"), Keeps::CantTell),
            (folder("a"), Lands::CantTell, Keeps::CantTell),
        ];
        for (was, now, keeps) in rows {
            assert_eq!(compare(&was, &now), keeps, "{was:?} then {now:?}");
        }
    }
}
