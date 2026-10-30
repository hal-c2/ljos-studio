//! The data a list shows: a `ListModel`, written in the document or in a
//! document of its own (`model: Contacts {}`), whose `ListElement`s are
//! its rows and whose roles are the names their members bind; or a
//! `JsonListModel` (Ljós's own) reading a JSON file of the project, whose
//! records are its rows and whose roles are their keys.
//!
//! Only data the project holds is read: a model made by code (an array, a
//! request's answer) or read from a URL has roles nobody knows before it
//! runs.

use serde::{Deserialize, Serialize};

use crate::{Address, Error, Node, Session, flows::beside, outline::literal_string};
use qml_edit::Step;

const LIST_MODEL: &str = "ListModel";
const JSON_LIST_MODEL: &str = "JsonListModel";
const LIST_ELEMENT: &str = "ListElement";
/// The types that make one delegate for each row of a `model`.
const VIEWS: [&str; 5] = ["ListView", "GridView", "Repeater", "PathView", "TableView"];

/// The data an object sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Model {
    /// Its roles, in the order the rows first bind them.
    pub roles: Vec<String>,
    pub rows: usize,
    /// The document the model is written in.
    pub doc: String,
    /// The JSON file a `JsonListModel` reads, relative to the project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The handle of the list whose delegate the object is in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}

fn short(ty: &str) -> &str {
    ty.rsplit('.').next().unwrap_or(ty)
}

/// The roles and rows of the `ListModel` at `model` in `tree`.
fn read(tree: &[(String, Node)], model: &Node, doc: &str) -> Model {
    let mut roles: Vec<String> = Vec::new();
    let mut rows = 0;
    for (_, n) in tree {
        let row = n.path.0.len() == model.path.0.len() + 1
            && n.path.0.starts_with(&model.path.0)
            && matches!(n.path.0.last(), Some(Step::Child(_)))
            && short(&n.ty) == LIST_ELEMENT;
        if !row {
            continue;
        }
        rows += 1;
        for b in &n.bindings {
            if !b.declared && !roles.contains(&b.name) {
                roles.push(b.name.clone());
            }
        }
    }
    Model {
        roles,
        rows,
        doc: doc.to_owned(),
        source: None,
        view: None,
    }
}

/// A JSON object's members in the order written, which are a row's roles
/// in the order a person reads them; a map would sort them.
pub struct Record(pub Vec<(String, serde_json::Value)>);

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Members;
        impl<'de> serde::de::Visitor<'de> for Members {
            type Value = Record;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a record")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Record, A::Error> {
                let mut out = Vec::new();
                while let Some(member) = map.next_entry()? {
                    out.push(member);
                }
                Ok(Record(out))
            }
        }
        d.deserialize_map(Members)
    }
}

/// The records of a JSON array of objects, which a `JsonListModel` makes
/// its rows.
///
/// # Errors
/// Text that is not such an array.
pub fn records(json: &str) -> Result<Vec<Record>, serde_json::Error> {
    serde_json::from_str(json)
}

/// The project path a QML file `doc` names with the relative URL `url`;
/// `None` for one with a scheme or a root, or one that climbs out of the
/// project, which the project does not hold.
fn resolve_url(doc: &str, url: &str) -> Option<String> {
    if url.contains(':') || url.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = doc.split('/').collect();
    parts.pop();
    for part in url.split('/') {
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

/// The relative URL a binding writes: a string, or a string the file
/// resolves against itself (`Qt.resolvedUrl("…")`), which is the same URL.
fn url_written(value: &str) -> Option<String> {
    let inner = value
        .strip_prefix("Qt.resolvedUrl(")
        .and_then(|v| v.strip_suffix(')'))
        .map_or(value, str::trim);
    literal_string(inner)
}

/// `to`, a project path, as a URL relative to the QML file `doc`.
pub fn relative_url(doc: &str, to: &str) -> String {
    let from: Vec<&str> = doc.split('/').collect();
    let from = &from[..from.len() - 1];
    let to: Vec<&str> = to.split('/').collect();
    let shared = from
        .iter()
        .zip(&to[..to.len() - 1])
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<&str> = vec![".."; from.len() - shared];
    out.extend(&to[shared..]);
    out.join("/")
}

impl Session {
    /// The data the object `address` names sees: a `ListModel`'s own, a
    /// list's model's, or, inside a list's delegate, the list's. `None`
    /// when it sees none, or none written as QML.
    ///
    /// # Errors
    /// An address that names nothing, or a document that does not parse.
    pub fn model(&mut self, path: &str, address: &Address) -> Result<Option<Model>, Error> {
        let (_, node) = self.resolve(path, address)?;
        let (tree, _) = self.tree(path)?;
        if short(&node.ty) == LIST_MODEL {
            return Ok(Some(read(&tree, &node, path)));
        }
        if short(&node.ty) == JSON_LIST_MODEL {
            return Ok(self.read_json(&node, path));
        }
        if VIEWS.contains(&short(&node.ty)) {
            return self.model_of(path, &tree, &node);
        }
        // The nearest list whose delegate the object is in. Its model, its
        // `header`, `footer` or `highlight` are made once, not for each row,
        // and see no row's roles: those see an outer list's, if any.
        for depth in (0..node.path.0.len()).rev() {
            let Some((handle, view)) = tree
                .iter()
                .find(|(_, n)| n.path.0 == node.path.0[..depth] && VIEWS.contains(&short(&n.ty)))
            else {
                continue;
            };
            let delegate = match &node.path.0[depth] {
                Step::Value(name) => name == "delegate",
                // A Repeater's default property is its delegate; the other
                // views' children are their content, not rows.
                Step::Child(_) => short(&view.ty) == "Repeater",
                _ => false,
            };
            if !delegate {
                continue;
            }
            let handle = handle.clone();
            let view = view.clone();
            return Ok(self.model_of(path, &tree, &view)?.map(|m| Model {
                view: Some(handle),
                ..m
            }));
        }
        Ok(None)
    }

    /// The project document that declares the type `ty` as `path` sees it:
    /// one beside it, or one an imported module of the project lists in its
    /// `qmldir`. Mock modules are the preview's, not the app's.
    fn type_document(&mut self, path: &str, ty: &str) -> Result<Option<String>, Error> {
        if ty.contains('.') {
            return Ok(None);
        }
        let near = beside(path, &format!("{ty}.qml"));
        if self.store().exists(&near) {
            return Ok(Some(near));
        }
        let text = self.text(path)?.0.to_owned();
        let imported = |module: &str| {
            text.lines().any(|l| {
                let mut words = l.split_whitespace();
                words.next() == Some("import") && words.next() == Some(module)
            })
        };
        for qmldir in self.store().list("qmldir") {
            if qmldir.starts_with("mocks/") || !qmldir.ends_with("qmldir") {
                continue;
            }
            let Ok(listing) = self.store().read(&qmldir) else {
                continue;
            };
            let module = listing.lines().find_map(|l| {
                let mut words = l.split_whitespace();
                (words.next() == Some("module"))
                    .then(|| words.next())
                    .flatten()
            });
            if !module.is_some_and(imported) {
                continue;
            }
            let file = listing.lines().find_map(|l| {
                let words: Vec<&str> = l.split_whitespace().collect();
                match words.as_slice() {
                    [name, _version, file] if *name == ty => Some(*file),
                    _ => None,
                }
            });
            if let Some(file) = file {
                let dir = qmldir.strip_suffix("qmldir").unwrap_or_default();
                return Ok(Some(format!("{dir}{file}")));
            }
        }
        Ok(None)
    }

    /// The `ListModel` a list's `model` names: one written as its value,
    /// one with that `id`, or a document beside this one whose root is one.
    fn model_of(
        &mut self,
        path: &str,
        tree: &[(String, Node)],
        view: &Node,
    ) -> Result<Option<Model>, Error> {
        let mut at = view.path.clone();
        at.0.push(Step::Value("model".to_owned()));
        if let Some((_, value)) = tree.iter().find(|(_, n)| n.path == at) {
            if short(&value.ty) == LIST_MODEL {
                return Ok(Some(read(tree, value, path)));
            }
            if short(&value.ty) == JSON_LIST_MODEL {
                return Ok(self.read_json(value, path));
            }
            let Some(doc) = self.type_document(path, &value.ty)? else {
                return Ok(None);
            };
            let (other, _) = match self.tree(&doc) {
                Ok(tree) => tree,
                Err(Error::Broken { .. }) => return Ok(None),
                Err(e) => return Err(e),
            };
            return Ok(match other.first() {
                Some((_, root)) if short(&root.ty) == LIST_MODEL => Some(read(&other, root, &doc)),
                Some((_, root)) if short(&root.ty) == JSON_LIST_MODEL => self.read_json(root, &doc),
                _ => None,
            });
        }
        let Some(binding) = view.bindings.iter().find(|b| b.name == "model") else {
            return Ok(None);
        };
        let id = binding.value.trim();
        let Some((_, model)) = tree.iter().find(|(_, n)| n.id.as_deref() == Some(id)) else {
            return Ok(None);
        };
        Ok(match short(&model.ty) {
            LIST_MODEL => Some(read(tree, model, path)),
            JSON_LIST_MODEL => self.read_json(model, path),
            _ => None,
        })
    }

    /// The roles and rows of the `JsonListModel` `model` of the document
    /// `doc`, read from its file now: the file is the data, which anything
    /// may have saved since. `None` when it names no file of the project
    /// (`json` text, a URL, a binding) or the file is not an array of
    /// records, whose roles are then not known.
    fn read_json(&self, model: &Node, doc: &str) -> Option<Model> {
        let binding = model.bindings.iter().find(|b| b.name == "source")?;
        let source = resolve_url(doc, &url_written(binding.value.trim())?)?;
        let text = self.store().read(&source).ok()?;
        let records = records(&text).ok()?;
        let mut roles: Vec<String> = Vec::new();
        for record in &records {
            for (key, _) in &record.0 {
                if !roles.contains(key) {
                    roles.push(key.clone());
                }
            }
        }
        Some(Model {
            roles,
            rows: records.len(),
            doc: doc.to_owned(),
            source: Some(source),
            view: None,
        })
    }
}
