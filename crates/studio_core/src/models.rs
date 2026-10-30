//! The data a list shows: a `ListModel`, written in the document or in a
//! document of its own (`model: Contacts {}`), whose `ListElement`s are
//! its rows and whose roles are the names their members bind.
//!
//! Only data written as QML is read: a model made by code (an array, a
//! request's answer) has roles nobody knows before it runs.

use serde::Serialize;

use crate::{Address, Error, Node, Session, flows::beside};
use qml_edit::Step;

const LIST_MODEL: &str = "ListModel";
const LIST_ELEMENT: &str = "ListElement";
/// The types that make one delegate for each row of a `model`.
const VIEWS: [&str; 5] = ["ListView", "GridView", "Repeater", "PathView", "TableView"];

/// The data an object sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Model {
    /// Its roles, in the order the rows first bind them.
    pub roles: Vec<String>,
    pub rows: usize,
    /// The document the `ListModel` is written in.
    pub doc: String,
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
        view: None,
    }
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
        if VIEWS.contains(&short(&node.ty)) {
            return self.model_of(path, &tree, &node);
        }
        // The nearest list the object is in, unless it is in the list's
        // model rather than its delegate.
        for depth in (0..node.path.0.len()).rev() {
            let Some((handle, view)) = tree
                .iter()
                .find(|(_, n)| n.path.0 == node.path.0[..depth] && VIEWS.contains(&short(&n.ty)))
            else {
                continue;
            };
            if node.path.0[depth] == Step::Value("model".to_owned()) {
                return Ok(None);
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
            let Some(doc) = self.type_document(path, &value.ty)? else {
                return Ok(None);
            };
            let (other, _) = match self.tree(&doc) {
                Ok(tree) => tree,
                Err(Error::Broken { .. }) => return Ok(None),
                Err(e) => return Err(e),
            };
            return Ok(other
                .first()
                .filter(|(_, root)| short(&root.ty) == LIST_MODEL)
                .map(|(_, root)| read(&other, root, &doc)));
        }
        let Some(binding) = view.bindings.iter().find(|b| b.name == "model") else {
            return Ok(None);
        };
        let id = binding.value.trim();
        Ok(tree
            .iter()
            .find(|(_, n)| n.id.as_deref() == Some(id) && short(&n.ty) == LIST_MODEL)
            .map(|(_, n)| read(tree, n, path)))
    }
}
