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

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

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

/// A property a person can give the object a value of, and what it can be.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Offer {
    /// As written: `wrapMode`, or `font.weight` for a part of a value.
    pub name: String,
    /// The type of the value, without its module; empty where only the name is known.
    #[serde(rename = "type")]
    pub ty: String,
    /// The values of its enum as a file writes them: `Text.WordWrap`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(default)]
    pub values: Vec<String>,
}

#[derive(Default)]
struct Found {
    offers: Vec<Offer>,
    signals: Vec<Signal>,
    properties: Vec<String>,
    /// What a project file or a type dump declares, which a person has no
    /// other way to learn: the toolkit's own are in its documentation, and
    /// there are too many to list on every object.
    taken: Vec<String>,
    not_text: Vec<String>,
    /// The toolkit types `properties` and `signals` were read from, whose
    /// table says less than the types have.
    from_table: Vec<&'static str>,
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

/// What Studio can say of `node`'s type, all from the one lookup, which reads
/// every type dump and `qmldir` of the project.
#[derive(Default)]
pub struct TypeSays {
    /// Every signal it has, a derived type's before its base's; `None` when
    /// its type is one Studio finds described nowhere, so that what it has is
    /// not known.
    pub signals: Option<Vec<Signal>>,
    /// What its project says it takes: the properties its files and type dumps
    /// declare that may be written from outside, the nearest first. Known even
    /// when what the type inherits is not.
    pub takes: Vec<String>,
    /// Those of them no string can be written to.
    pub not_text: Vec<String>,
    /// What it can be given, nearest first, whether written or not: a person
    /// should not need the documentation open to find a property's name, nor
    /// the spelling of its values.
    pub offers: Vec<Offer>,
}

/// All that of `node` of the document `doc`, whose text is `source`.
pub fn described(store: &dyn Store, doc: &str, source: &str, node: &Node) -> TypeSays {
    let Some((mut found, whole)) = look_up(store, doc, source, node) else {
        return TypeSays::default();
    };
    let mut takes: Vec<String> = Vec::new();
    for name in &found.taken {
        if !takes.contains(name) {
            takes.push(name.clone());
        }
    }
    let mut offers: Vec<Offer> = takes
        .iter()
        .map(|name| Offer {
            name: name.clone(),
            ty: String::new(),
            values: Vec::new(),
        })
        .collect();
    for offer in &found.offers {
        if !offers.iter().any(|o| o.name == offer.name) {
            offers.push(offer.clone());
        }
    }
    TypeSays {
        not_text: std::mem::take(&mut found.not_text),
        signals: (whole == Described::Whole).then(|| found.done()),
        takes,
        offers,
    }
}

/// `described`'s signals alone.
pub fn of(store: &dyn Store, doc: &str, source: &str, node: &Node) -> Option<Vec<Signal>> {
    described(store, doc, source, node).signals
}

fn look_up(store: &dyn Store, doc: &str, source: &str, node: &Node) -> Option<(Found, Described)> {
    let types = types_of(store);
    let scope = Scope {
        dir: dir_of(doc).to_owned(),
        imports: studio_core::outline::imports(source)?,
    };
    let mut found = Found::default();
    found.declared(node);
    let described = types.lookup(&scope, &node.ty, &mut found, 0);
    Some((found, described))
}

/// What a type's name is, in a document.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TypeIs {
    Known,
    /// Nothing the document imports has it, and all of those are described.
    Missing,
    /// A module it imports is described nowhere and may have it: only
    /// running the document could tell.
    Unsure,
}

mod lookup;
pub(crate) use lookup::{Keeps, Lands, Unvouched, compare};

/// Whether a folder import is one in the project, whose files can be read.
fn readable_folder(dir: &str, path: &str) -> bool {
    !path.starts_with('/') && !path.contains(':') && crate::blocks::join(dir, path).is_some()
}

/// What a document can name, read once for all a write asks of it.
pub(crate) struct Names<'s> {
    types: Types<'s>,
    scope: Scope,
    source: String,
}

impl<'s> Names<'s> {
    /// For `doc`, whose text is `source`, once it imports `added` too. `None`
    /// when the text does not read: the write is the one to refuse that.
    pub(crate) fn of(
        store: &'s dyn Store,
        doc: &str,
        source: &str,
        added: &[String],
    ) -> Option<Self> {
        let mut imports = studio_core::outline::imports(source)?;
        // A qualified import of it writes the type qualified; bare is how the
        // text being checked names it either way.
        imports.extend(added.iter().map(|uri| {
            if uri.contains('/') {
                Import::Path {
                    path: uri.clone(),
                    alias: None,
                }
            } else {
                Import::Module {
                    uri: uri.clone(),
                    alias: None,
                }
            }
        }));
        Some(Self {
            types: types_of(store),
            scope: Scope {
                dir: dir_of(doc).to_owned(),
                imports,
            },
            source: source.to_owned(),
        })
    }

    /// For the file `doc`, with what it imports already read.
    pub(crate) fn at(store: &'s dyn Store, doc: &str, imports: &[Import]) -> Self {
        Self {
            types: types_of(store),
            scope: Scope {
                dir: dir_of(doc).to_owned(),
                imports: imports.to_vec(),
            },
            source: String::new(),
        }
    }

    /// `ty` as this document, plus the object text `beside` it, names it. It
    /// is `Missing` only when qml6 was seen to refuse it: every import is one
    /// whose types are all known, and the type was tried in the toolkit's.
    pub(crate) fn type_is(&self, ty: &str, beside: &str) -> TypeIs {
        let found = &mut Found::default();
        if self.types.lookup(&self.scope, ty, found, 0) != Described::Not {
            return TypeIs::Known;
        }
        let inline = format!("component {ty}");
        // A type the file uses already, another of changes nothing about
        // whether it loads.
        if studio_core::outline::outline(&self.source)
            .is_some_and(|nodes| nodes.iter().any(|node| node.ty == ty))
            || self.source.contains(&inline)
            || beside.contains(&inline)
        {
            return TypeIs::Known;
        }
        if let Some((qualifier, _)) = ty.rsplit_once('.') {
            let aliased = |import: &Import| match import {
                Import::Module { alias, .. } | Import::Path { alias, .. } => {
                    alias.as_deref() == Some(qualifier)
                }
            };
            // `Other.Inner` names what a file or import called `Other` holds.
            let names_type = self.types.lookup(&self.scope, qualifier, found, 0) != Described::Not;
            return if names_type || self.scope.imports.iter().any(aliased) || self.undescribed() {
                TypeIs::Unsure
            } else {
                TypeIs::Missing
            };
        }
        let Some(modules) = measured_types().get(ty) else {
            return TypeIs::Unsure;
        };
        let has_it = self.scope.imports.iter().any(|import| {
            matches!(import, Import::Module { uri, alias: None } if modules.contains(uri.as_str()))
        });
        if has_it {
            TypeIs::Known
        } else if self.undescribed() {
            TypeIs::Unsure
        } else {
            TypeIs::Missing
        }
    }

    /// Whether the document imports a module whose types nothing here knows.
    fn undescribed(&self) -> bool {
        self.scope.imports.iter().any(|import| self.unread(import))
    }

    fn unread(&self, import: &Import) -> bool {
        match import {
            Import::Module { uri, .. } => {
                !MEASURED_MODULES.contains(&uri.as_str())
                    && self.types.dumps.module_types(uri).is_none()
                    && !self.types.qmldirs.contains_key(uri)
            }
            Import::Path { path, .. } => !readable_folder(&self.scope.dir, path),
        }
    }

    /// Whether an object of type `ty`, declaring what `node` does, certainly
    /// has no member `name`, or signal for a handler `name`. An attached
    /// property is not one of its own, and what isn't wholly described may
    /// have it.
    pub(crate) fn lacks(&self, ty: &str, node: Option<&Node>, name: &str) -> bool {
        let base = name.split('.').next().unwrap_or(name);
        let handled = base.strip_prefix("on").and_then(|rest| {
            let first = rest.chars().next().filter(char::is_ascii_uppercase)?;
            Some(format!("{}{}", first.to_ascii_lowercase(), &rest[1..]))
        });
        if base == "id" || base.starts_with(|c: char| c.is_ascii_uppercase()) {
            return false;
        }
        let mut found = Found::default();
        if let Some(node) = node {
            found.declared(node);
        }
        if self.types.lookup(&self.scope, ty, &mut found, 0) != Described::Whole {
            return false;
        }
        let Some(also) = found
            .from_table
            .iter()
            .map(|ty| measured().get(ty))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let also = |name: &str| also.iter().any(|members| members.contains(name));
        if also(if handled.is_some() { "on*" } else { "*" }) {
            return false;
        }
        let property = |name: &str| found.properties.iter().any(|p| p == name) || also(name);
        match handled {
            Some(said) => {
                !found.signals.iter().any(|s| s.name == said)
                    && !also(&said)
                    && !said.strip_suffix("Changed").is_some_and(property)
            }
            None => !property(base),
        }
    }
}

/// The modules whose types were tried: only of these is a type's absence seen.
const MEASURED_MODULES: [&str; 4] = ["QtQml", "QtQuick", "QtQuick.Layouts", "QtQuick.Controls"];

/// The words qml6 was seen to refuse as an id: `mise run measure-members`.
pub(crate) fn not_an_id(word: &str) -> bool {
    include_str!("measured_ids.txt")
        .lines()
        .any(|line| line == word)
}

/// Each type tried, and which of `MEASURED_MODULES` have it.
fn measured_types() -> &'static HashMap<&'static str, HashSet<&'static str>> {
    static MEASURED: OnceLock<HashMap<&str, HashSet<&str>>> = OnceLock::new();
    MEASURED.get_or_init(|| {
        include_str!("measured_types.txt")
            .lines()
            .filter_map(|line| line.split_once(' '))
            .map(|(ty, modules)| (ty, modules.split(',').collect()))
            .collect()
    })
}

/// What qml6 says each toolkit type has beyond the table: `mise run
/// measure-members`. `*` and `on*` mean it took a made-up property or handler.
/// A type with no entry was not measured, and is taken to have anything.
fn measured() -> &'static HashMap<&'static str, HashSet<&'static str>> {
    static MEASURED: OnceLock<HashMap<&str, HashSet<&str>>> = OnceLock::new();
    MEASURED.get_or_init(|| {
        include_str!("measured_members.txt")
            .lines()
            .filter_map(|line| line.split_once(' '))
            .map(|(ty, members)| (ty, members.split(',').collect()))
            .collect()
    })
}

fn types_of(store: &dyn Store) -> Types<'_> {
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
    Types {
        store,
        dumps,
        qmldirs,
    }
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
                found.offers.extend(crate::toolkit::offers(uri, name));
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
            found.not_text.extend(
                ty.properties
                    .iter()
                    .filter(|p| p.list || !studio_core::outline::takes_string(&p.ty))
                    .map(|p| p.name.clone()),
            );
            found.taken.extend(
                ty.properties
                    .iter()
                    .filter(|p| !p.readonly)
                    .map(|p| p.name.clone()),
            );
            if let Some(base) = self.dumps.base_of(ty) {
                ty = base;
                continue;
            }
            let Some(base) = &ty.base else { return true };
            if let Some(m) = &ty.base_module {
                module = m.clone();
            }
            return match qt_signals::module(&module).and_then(|m| m.type_named(base)) {
                Some(found_base) => {
                    toolkit(found_base, found);
                    found.offers.extend(crate::toolkit::offers(&module, base));
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
            // A block is announced in a comment, with its category and summary
            // after the file.
            let file = text.lines().find_map(|line| {
                if let Some(block) = line.trim_start().strip_prefix("# block ") {
                    let words: Vec<&str> = block.split_whitespace().collect();
                    return match words.as_slice() {
                        [n, _, file, ..] if *n == name && file.ends_with(".qml") => {
                            Some(file.to_string())
                        }
                        _ => None,
                    };
                }
                let words: Vec<&str> = line.split_whitespace().collect();
                match words.as_slice() {
                    [n, file] | [n, _, file] if *n == name && file.ends_with(".qml") => {
                        Some(file.to_string())
                    }
                    _ => None,
                }
            })?;
            crate::blocks::join(dir_of(qmldir), &file)
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
        found.not_text.extend(root.not_text.iter().cloned());
        found.taken.extend(
            root.properties
                .iter()
                .filter(|p| !root.read_only.contains(p))
                .cloned(),
        );
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
    found.from_table.push(ty.name);
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

/// Input of `mise run measure-members`: what the table says, type by type.
#[cfg(test)]
mod table {
    use std::fmt::Write;

    #[test]
    #[ignore = "writes a file for mise run measure-members"]
    fn a_table_to_measure() {
        let mut out = String::new();
        for uri in ["QtQml", "QtQuick", "QtQuick.Layouts", "QtQuick.Controls"] {
            let Some(module) = qt_signals::module(uri) else {
                continue;
            };
            for name in crate::toolkit::names(uri).unwrap_or_default() {
                let Some(ty) = module.type_named(&name) else {
                    continue;
                };
                let mut found = super::Found::default();
                super::toolkit(ty, &mut found);
                let signals: Vec<_> = found.signals.iter().map(|s| s.name.as_str()).collect();
                let properties = found.properties.join(",");
                let signals = signals.join(",");
                writeln!(out, "{uri}\t{name}\t{properties}\t{signals}").unwrap();
            }
        }
        std::fs::write(std::env::var("TOOLKIT_TABLE").unwrap(), out).unwrap();
    }
}
