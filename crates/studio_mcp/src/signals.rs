//! The signals an object has, which a flow's handler may name: what its
//! type declares and inherits, as the project's type dumps (a block written
//! in Rust), its QML files and Ljós's own table of types describe them, and
//! what the object declares itself.
//!
//! A type is looked up as the document imports it: a qualified name
//! (`Controls.Button`) in the imports of that name, a bare one in each
//! import without one in the order written, then in the document's
//! directory. In a module, a dump comes first (a Rust type wins over a QML
//! one of the same name, as in [`crate::blocks`]), then a project `qmldir`,
//! then the table.

use ljos_blocks::{DumpedType, DumpedTypes};
use schemars::JsonSchema;
use serde::Serialize;
use studio_core::{Import, Node, Store};

/// One signal, which `on<Name>` handles.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Signal {
    pub name: String,
    /// What a handler may call its arguments, in order, each with its type
    /// (empty where nothing says it). An argument without a name has an
    /// empty one.
    pub arguments: Vec<Argument>,
    /// A property's change signal (`widthChanged`): every property has one,
    /// so a person picking what a tap does looks past them.
    pub change: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Argument {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// A chain of types that comes back to itself, or one longer than any a
/// project writes, ends here.
const DEPTH: usize = 32;

/// Where a type name is looked up: a document's imports and directory.
struct Scope {
    dir: String,
    imports: Vec<Import>,
}

#[derive(Default)]
struct Found {
    signals: Vec<Signal>,
    properties: Vec<String>,
}

impl Found {
    fn add(&mut self, name: &str, arguments: Vec<Argument>) {
        if !self.signals.iter().any(|s| s.name == name) {
            self.signals.push(Signal {
                name: name.to_owned(),
                arguments,
                change: false,
            });
        }
    }

    fn declared(&mut self, node: &Node) {
        for s in &node.signals {
            let arguments = s
                .arguments
                .iter()
                .map(|(name, ty)| Argument {
                    name: name.clone(),
                    ty: ty.clone(),
                })
                .collect();
            self.add(&s.name, arguments);
        }
        for p in &node.properties {
            self.add(&format!("{p}Changed"), Vec::new());
            self.properties.push(p.clone());
        }
    }

    fn done(mut self) -> Vec<Signal> {
        for s in &mut self.signals {
            s.change = s
                .name
                .strip_suffix("Changed")
                .is_some_and(|p| self.properties.iter().any(|q| q == p));
        }
        self.signals
    }
}

/// The project as a type is looked up in it.
struct Types<'s> {
    store: &'s dyn Store,
    dumps: DumpedTypes,
}

/// Every signal `node` of the document `doc` (whose text is `source`) has,
/// a derived type's before its base's; `None` when its type is one Studio
/// finds described nowhere, so that what it has is not known.
pub fn of(store: &dyn Store, doc: &str, source: &str, node: &Node) -> Option<Vec<Signal>> {
    let mut dumps = DumpedTypes::default();
    for file in crate::blocks::dumps(store) {
        // A dump that does not read is `blocks_list`'s to report; its types
        // are then described nowhere.
        if let Ok(text) = store.read(&file) {
            let _ = dumps.add_dump(&text);
        }
    }
    let types = Types { store, dumps };
    let scope = Scope {
        dir: dir_of(doc).to_owned(),
        imports: studio_core::outline::imports(source)?,
    };
    let mut found = Found::default();
    found.declared(node);
    types
        .lookup(&scope, &node.ty, &mut found, 0)
        .then(|| found.done())
}

fn dir_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

impl Types<'_> {
    /// Adds the signals of the type `name` is in `scope`; false when no
    /// description of it is found.
    fn lookup(&self, scope: &Scope, name: &str, found: &mut Found, depth: usize) -> bool {
        if depth > DEPTH {
            return false;
        }
        let (qualifier, bare) = match name.rsplit_once('.') {
            Some((q, n)) => (Some(q), n),
            None => (None, name),
        };
        for import in &scope.imports {
            match import {
                Import::Module { uri, alias } if alias.as_deref() == qualifier => {
                    if self.in_module(uri, bare, found, depth) {
                        return true;
                    }
                }
                Import::Path { path, alias }
                    if alias.as_deref() == qualifier && !path.ends_with(".js") =>
                {
                    if let Some(dir) = crate::blocks::join(&scope.dir, path)
                        && self.in_dir(&dir, bare, found, depth)
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        qualifier.is_none() && self.in_dir(&scope.dir, bare, found, depth)
    }

    fn in_module(&self, uri: &str, name: &str, found: &mut Found, depth: usize) -> bool {
        if let Some(ty) = self.dumps.type_named(uri, name) {
            self.dumped(uri, ty, found, depth);
            return true;
        }
        if let Some(file) = self.qmldir_file(uri, name) {
            return self.file(&file, found, depth);
        }
        match qt_signals::module(uri).and_then(|m| m.type_named(name)) {
            Some(ty) => {
                toolkit(ty, found);
                true
            }
            None => false,
        }
    }

    /// A type a dump describes, and what it inherits: from the dumps while
    /// they describe its bases, then from the toolkit.
    fn dumped<'a>(&'a self, uri: &str, mut ty: &'a DumpedType, found: &mut Found, depth: usize) {
        let mut module = uri.to_owned();
        for _ in depth..=DEPTH {
            for s in ty.own_signals() {
                let arguments = s
                    .parameters
                    .iter()
                    .map(|p| Argument {
                        name: p.name.clone(),
                        ty: p.ty.clone(),
                    })
                    .collect();
                found.add(&s.name, arguments);
            }
            found
                .properties
                .extend(ty.properties.iter().map(|p| p.name.clone()));
            if let Some(base) = self.dumps.base_of(ty) {
                ty = base;
                continue;
            }
            let Some(base) = &ty.base else { return };
            if let Some(m) = &ty.base_module {
                module = m.clone();
            }
            if let Some(base) = qt_signals::module(&module).and_then(|m| m.type_named(base)) {
                toolkit(base, found);
            }
            return;
        }
    }

    /// The QML file a project `qmldir` of the module `uri` names `name`.
    fn qmldir_file(&self, uri: &str, name: &str) -> Option<String> {
        self.store
            .list("qmldir")
            .into_iter()
            .filter(|p| p.rsplit('/').next() == Some("qmldir"))
            .find_map(|qmldir| {
                let text = self.store.read(&qmldir).ok()?;
                let mut lines = text.lines().map(str::split_whitespace);
                let module = lines.clone().find_map(|mut words| {
                    (words.next() == Some("module")).then(|| words.next())?
                })?;
                if module != uri {
                    return None;
                }
                // `Name 1.0 File.qml`, or `Name File.qml` without a version.
                let file = lines.find_map(|words| {
                    let words: Vec<&str> = words.collect();
                    match words.as_slice() {
                        [n, file] | [n, _, file] if *n == name && file.ends_with(".qml") => {
                            Some(*file)
                        }
                        _ => None,
                    }
                })?;
                crate::blocks::join(dir_of(&qmldir), file)
            })
    }

    fn in_dir(&self, dir: &str, name: &str, found: &mut Found, depth: usize) -> bool {
        let file = crate::blocks::join(dir, &format!("{name}.qml"));
        file.is_some_and(|file| self.file(&file, found, depth))
    }

    /// A component a QML file is: what its root declares, and its root's
    /// type as the file imports it.
    fn file(&self, file: &str, found: &mut Found, depth: usize) -> bool {
        let Ok(source) = self.store.read(file) else {
            return false;
        };
        let (Some(nodes), Some(imports)) = (
            studio_core::outline::outline(&source),
            studio_core::outline::imports(&source),
        ) else {
            return false;
        };
        let Some(root) = nodes.first() else {
            return false;
        };
        found.declared(root);
        let scope = Scope {
            dir: dir_of(file).to_owned(),
            imports,
        };
        self.lookup(&scope, &root.ty, found, depth + 1)
    }
}

/// A toolkit type, from Ljós's table, and what it inherits.
fn toolkit(ty: &'static qt_signals::Type, found: &mut Found) {
    let mut ty = Some(ty);
    for _ in 0..=DEPTH {
        let Some(t) = ty else { return };
        for s in &t.signals {
            let arguments = s
                .parameters
                .iter()
                .enumerate()
                .map(|(i, name)| Argument {
                    name: if *name == "_" { "" } else { name }.to_owned(),
                    ty: s.parameter_types.get(i).copied().unwrap_or("").to_owned(),
                })
                .collect();
            found.add(s.name, arguments);
        }
        found
            .properties
            .extend(t.properties.iter().map(|p| p.name.to_owned()));
        ty = t.prototype();
    }
}
