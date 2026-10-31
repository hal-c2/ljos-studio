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

use std::collections::HashMap;

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

/// What a lookup learned of a type.
#[derive(Clone, Copy, PartialEq)]
enum Described {
    /// Nothing here is this type: the lookup goes on to the next place.
    Not,
    /// This is the type, but what it inherits is not known. The lookup stops,
    /// as another type of the name is not this one.
    InPart,
    Whole,
}

/// The project as a type is looked up in it, read once per lookup: a chain
/// of bases asks for modules many times, and each walk of the project
/// would be paid again at every level.
struct Types<'s> {
    store: &'s dyn Store,
    dumps: DumpedTypes,
    /// Each project `qmldir` that declares a module: its path and text, by
    /// the module's URI.
    qmldirs: HashMap<String, Vec<(String, String)>>,
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
    let mut qmldirs: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for qmldir in store.list("qmldir") {
        if qmldir.rsplit('/').next() != Some("qmldir") {
            continue;
        }
        let Ok(text) = store.read(&qmldir) else {
            continue;
        };
        let module = text
            .lines()
            .map(str::split_whitespace)
            .find_map(|mut words| (words.next() == Some("module")).then(|| words.next())?);
        if let Some(module) = module {
            qmldirs
                .entry(module.to_owned())
                .or_default()
                .push((qmldir, text));
        }
    }
    let types = Types {
        store,
        dumps,
        qmldirs,
    };
    let scope = Scope {
        dir: dir_of(doc).to_owned(),
        imports: studio_core::outline::imports(source)?,
    };
    let mut found = Found::default();
    found.declared(node);
    (types.lookup(&scope, &node.ty, &mut found, 0) == Described::Whole).then(|| found.done())
}

fn dir_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

impl Types<'_> {
    /// Adds the signals of the type `name` is in `scope`, and says how much
    /// of it was described. The first place that has the type ends the
    /// lookup, whatever it knows of it.
    fn lookup(&self, scope: &Scope, name: &str, found: &mut Found, depth: usize) -> Described {
        if depth > DEPTH {
            return Described::InPart;
        }
        let (qualifier, bare) = match name.rsplit_once('.') {
            Some((q, n)) => (Some(q), n),
            None => (None, name),
        };
        for import in &scope.imports {
            match import {
                Import::Module { uri, alias } if alias.as_deref() == qualifier => {
                    let described = self.in_module(uri, bare, found, depth);
                    if described != Described::Not {
                        return described;
                    }
                }
                Import::Path { path, alias }
                    if alias.as_deref() == qualifier && !path.ends_with(".js") =>
                {
                    if let Some(dir) = crate::blocks::join(&scope.dir, path) {
                        let described = self.in_dir(&dir, bare, found, depth);
                        if described != Described::Not {
                            return described;
                        }
                    }
                }
                _ => {}
            }
        }
        if qualifier.is_some() {
            return Described::Not;
        }
        self.in_dir(&scope.dir, bare, found, depth)
    }

    fn in_module(&self, uri: &str, name: &str, found: &mut Found, depth: usize) -> Described {
        if let Some(ty) = self.dumps.type_named(uri, name) {
            return if self.dumped(uri, ty, found, depth) {
                Described::Whole
            } else {
                Described::InPart
            };
        }
        if let Some(file) = self.qmldir_file(uri, name) {
            return self.file(&file, found, depth);
        }
        match qt_signals::module(uri).and_then(|m| m.type_named(name)) {
            Some(ty) => {
                toolkit(ty, found);
                Described::Whole
            }
            None => Described::Not,
        }
    }

    /// A type a dump describes, and what it inherits: from the dumps while
    /// they describe its bases, then from the toolkit. False when a base it
    /// names is described nowhere: what it inherits is then not known.
    fn dumped<'a>(
        &'a self,
        uri: &str,
        mut ty: &'a DumpedType,
        found: &mut Found,
        depth: usize,
    ) -> bool {
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
            let Some(base) = &ty.base else { return true };
            if let Some(m) = &ty.base_module {
                module = m.clone();
            }
            return match qt_signals::module(&module).and_then(|m| m.type_named(base)) {
                Some(base) => {
                    toolkit(base, found);
                    true
                }
                None => false,
            };
        }
        false
    }

    /// The QML file a project `qmldir` of the module `uri` names `name`.
    fn qmldir_file(&self, uri: &str, name: &str) -> Option<String> {
        self.qmldirs.get(uri)?.iter().find_map(|(qmldir, text)| {
            // `Name 1.0 File.qml`, or `Name File.qml` without a version.
            let file = text.lines().map(str::split_whitespace).find_map(|words| {
                let words: Vec<&str> = words.collect();
                match words.as_slice() {
                    [n, file] | [n, _, file] if *n == name && file.ends_with(".qml") => Some(*file),
                    _ => None,
                }
            })?;
            crate::blocks::join(dir_of(qmldir), file)
        })
    }

    fn in_dir(&self, dir: &str, name: &str, found: &mut Found, depth: usize) -> Described {
        match crate::blocks::join(dir, &format!("{name}.qml")) {
            Some(file) => self.file(&file, found, depth),
            None => Described::Not,
        }
    }

    /// A component a QML file is: what its root declares, and its root's
    /// type as the file imports it. A file that is there is the type, even
    /// when it does not read as QML or its root's type is described nowhere.
    fn file(&self, file: &str, found: &mut Found, depth: usize) -> Described {
        let Ok(source) = self.store.read(file) else {
            return Described::Not;
        };
        let (Some(nodes), Some(imports)) = (
            studio_core::outline::outline(&source),
            studio_core::outline::imports(&source),
        ) else {
            return Described::InPart;
        };
        let Some(root) = nodes.first() else {
            return Described::InPart;
        };
        found.declared(root);
        let scope = Scope {
            dir: dir_of(file).to_owned(),
            imports,
        };
        match self.lookup(&scope, &root.ty, found, depth + 1) {
            Described::Whole => Described::Whole,
            Described::InPart | Described::Not => Described::InPart,
        }
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
