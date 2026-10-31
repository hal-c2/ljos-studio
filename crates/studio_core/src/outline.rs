//! A document's objects as a designer lists them: where each is, what it
//! is, what it is called and what it binds.

use ljos_ir::KeySeg;
use qml_edit::{NodePath, Step};
use qml_syntax::{
    Allocator, GetSpan,
    ast::{QmlBindingValue, QmlImportSource, QmlMember, QmlObject},
};
use serde::Serialize;

/// One object of a document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Node {
    #[serde(serialize_with = "path_str")]
    pub path: NodePath,
    #[serde(rename = "type")]
    pub ty: String,
    pub id: Option<String>,
    /// Its literal `objectName`, the next most stable name after `id`.
    #[serde(skip)]
    pub object_name: Option<String>,
    pub depth: usize,
    /// Byte range of the object in the text.
    pub start: usize,
    pub end: usize,
    /// One-based line the object starts on.
    pub line: usize,
    pub bindings: Vec<Binding>,
    /// The signals it declares (`signal picked(int index)`), in order.
    #[serde(skip)]
    pub signals: Vec<DeclaredSignal>,
    /// The properties it declares, valued or not: each has a change
    /// signal, `nameChanged`.
    #[serde(skip)]
    pub properties: Vec<String>,
    /// Its place as hot reload names it ([`ljos_ir::ObjectKey`]'s steps): the
    /// most stable step at each level from the root.
    #[serde(skip)]
    pub key: Vec<KeySeg>,
}

fn path_str<S: serde::Serializer>(path: &NodePath, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&path.to_string())
}

/// A binding or a property declaration with a value, as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub name: String,
    pub value: String,
    pub start: usize,
    pub end: usize,
    /// `property` declarations say so: they are part of the object's own
    /// interface, not settings of an inherited one.
    pub declared: bool,
}

/// A `signal` an object declares, each argument's name and type as
/// written (empty for one written without a type).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredSignal {
    pub name: String,
    pub arguments: Vec<(String, String)>,
}

/// One `import` of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Import {
    /// A module by its URI (`QtQuick.Controls`), and the name it is
    /// imported as (`as Controls`).
    Module { uri: String, alias: Option<String> },
    /// A directory or a JavaScript file, as the string names it.
    Path { path: String, alias: Option<String> },
}

/// The imports of `source` in the order written; `None` when it does not
/// parse.
pub fn imports(source: &str) -> Option<Vec<Import>> {
    let allocator = Allocator::default();
    let parsed = qml_syntax::parse(&allocator, source);
    if !parsed.is_ok() {
        return None;
    }
    let imports = parsed.document.imports.iter().map(|i| {
        let alias = i.alias.as_ref().map(|a| a.name.to_string());
        match &i.source {
            QmlImportSource::Module(name) => Import::Module {
                uri: name.to_string(),
                alias,
            },
            QmlImportSource::Path(path) => Import::Path {
                path: path.trim_matches(['"', '\'']).to_owned(),
                alias,
            },
        }
    });
    Some(imports.collect())
}

/// Whether a member written as an object is a grouped property
/// (`font { }`) rather than an object: a lowercase type name.
pub fn is_group(object: &QmlObject<'_>) -> bool {
    object
        .type_name
        .parts
        .last()
        .and_then(|p| p.chars().next())
        .is_some_and(|c| c.is_ascii_lowercase())
}

/// The objects of `source`, root first in the order written; `None` when it
/// does not parse.
pub fn outline(source: &str) -> Option<Vec<Node>> {
    let allocator = Allocator::default();
    let parsed = qml_syntax::parse(&allocator, source);
    if !parsed.is_ok() {
        return None;
    }
    let mut out = Vec::new();
    let mut key = vec![KeySeg::Typed {
        ty: parsed.document.root.type_name.to_string(),
        nth: 0,
    }];
    walk(
        source,
        &parsed.document.root,
        &mut NodePath::root(),
        &mut key,
        &mut out,
    );
    Some(out)
}

fn walk(
    source: &str,
    object: &QmlObject<'_>,
    path: &mut NodePath,
    key: &mut Vec<KeySeg>,
    out: &mut Vec<Node>,
) {
    let mut bindings = Vec::new();
    collect_bindings(source, object, "", &mut bindings);
    let id = bindings
        .iter()
        .find(|b| b.name == "id")
        .map(|b| b.value.clone());
    let object_name = bindings
        .iter()
        .find(|b| b.name == "objectName")
        .and_then(|b| literal_string(&b.value));
    if let Some(last) = key.last_mut() {
        if let Some(id) = &id {
            *last = KeySeg::Id(id.clone());
        } else if let Some(name) = &object_name {
            *last = KeySeg::Named(name.clone());
        }
    }
    out.push(Node {
        path: path.clone(),
        ty: object.type_name.to_string(),
        id,
        object_name,
        depth: path.0.len(),
        start: object.span.start as usize,
        end: object.span.end as usize,
        line: qml_syntax::position(source, object.span.start).0,
        bindings,
        signals: declared_signals(source, object),
        properties: object
            .members
            .iter()
            .filter_map(|m| match m {
                QmlMember::Property(p) => Some(p.name.name.to_string()),
                _ => None,
            })
            .collect(),
        key: key.clone(),
    });
    let mut child = 0;
    // Typed steps count siblings of one type that have neither id nor name.
    let mut typed: Vec<(String, u32)> = Vec::new();
    for member in &object.members {
        let (step, inner) = match member {
            QmlMember::Object(inner) if !is_group(inner) => {
                child += 1;
                (Step::Child(child - 1), inner)
            }
            QmlMember::Binding(b) => match &b.value {
                QmlBindingValue::Object(inner) if !is_group(inner) => {
                    (Step::Value(b.name.to_string()), inner)
                }
                _ => continue,
            },
            QmlMember::Property(p) => match &p.value {
                Some(QmlBindingValue::Object(inner)) => {
                    (Step::Value(p.name.name.to_string()), inner)
                }
                _ => continue,
            },
            QmlMember::InlineComponent(c) => (Step::Component(c.name.name.to_string()), &c.object),
            _ => continue,
        };
        let ty = inner.type_name.to_string();
        let nth = if is_named(source, inner) {
            0
        } else {
            match typed.iter_mut().find(|(t, _)| *t == ty) {
                Some((_, n)) => {
                    *n += 1;
                    *n - 1
                }
                None => {
                    typed.push((ty.clone(), 1));
                    0
                }
            }
        };
        path.0.push(step);
        key.push(KeySeg::Typed { ty, nth });
        walk(source, inner, path, key, out);
        key.pop();
        path.0.pop();
    }
}

fn declared_signals(source: &str, object: &QmlObject<'_>) -> Vec<DeclaredSignal> {
    let typed = |t: &Option<qml_syntax::ast::QmlTypeName<'_>>| {
        t.as_ref()
            .map_or_else(String::new, |t| qml_syntax::text(source, t.span).to_owned())
    };
    object
        .members
        .iter()
        .filter_map(|m| match m {
            QmlMember::Signal(s) => Some(DeclaredSignal {
                name: s.name.name.to_string(),
                arguments: s
                    .params
                    .iter()
                    .map(|p| (p.name.name.to_string(), typed(&p.type_name)))
                    .collect(),
            }),
            _ => None,
        })
        .collect()
}

/// Whether an object has an `id` or a literal `objectName`, so that its key
/// step is that name and it does not count among its unnamed siblings.
fn is_named(source: &str, object: &QmlObject<'_>) -> bool {
    object.members.iter().any(|m| match m {
        QmlMember::Binding(b) => {
            let name = b.name.to_string();
            name == "id"
                || (name == "objectName"
                    && matches!(&b.value, QmlBindingValue::Expression(e)
                        if literal_string(qml_syntax::text(source, e.span())).is_some()))
        }
        _ => false,
    })
}

pub(crate) fn collect_bindings(
    source: &str,
    object: &QmlObject<'_>,
    prefix: &str,
    out: &mut Vec<Binding>,
) {
    for member in &object.members {
        match member {
            QmlMember::Binding(b) => {
                let name = format!("{prefix}{}", b.name);
                match &b.value {
                    QmlBindingValue::Expression(e) => {
                        out.push(binding(source, name, e.span(), false))
                    }
                    QmlBindingValue::Statement(s) => {
                        out.push(binding(source, name, s.span(), false))
                    }
                    QmlBindingValue::Object(inner) if is_group(inner) => {
                        collect_bindings(source, inner, &format!("{name}."), out);
                    }
                    _ => {}
                }
            }
            QmlMember::Object(inner) if is_group(inner) => {
                collect_bindings(source, inner, &format!("{prefix}{}.", inner.type_name), out);
            }
            QmlMember::Property(p) => {
                if let Some(QmlBindingValue::Expression(e)) = &p.value {
                    out.push(binding(
                        source,
                        format!("{prefix}{}", p.name.name),
                        e.span(),
                        true,
                    ));
                }
            }
            _ => {}
        }
    }
}

fn binding(source: &str, name: String, span: qml_syntax::Span, declared: bool) -> Binding {
    Binding {
        name,
        value: qml_syntax::text(source, span).to_owned(),
        start: span.start as usize,
        end: span.end as usize,
        declared,
    }
}

pub(crate) fn literal_string(value: &str) -> Option<String> {
    let inner = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))?;
    (!inner.contains(['\\', '"', '\''])).then(|| inner.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_are_bindings_not_objects() {
        let nodes = outline(
            "Item {\n    id: root\n    font { bold: true }\n    Text { font.pixelSize: 3 }\n    Text { objectName: \"t\" }\n}\n",
        )
        .unwrap();
        let paths: Vec<String> = nodes.iter().map(|n| n.path.to_string()).collect();
        assert_eq!(paths, ["root", "root/0", "root/1"]);
        assert_eq!(nodes[0].bindings[1].name, "font.bold");
        assert_eq!(nodes[1].bindings[0].name, "font.pixelSize");
        assert_eq!(
            nodes[1].key,
            [
                KeySeg::Id("root".into()),
                KeySeg::Typed {
                    ty: "Text".into(),
                    nth: 0
                }
            ]
        );
        assert_eq!(nodes[2].key[1], KeySeg::Named("t".into()));
    }

    #[test]
    fn imports_are_modules_and_paths_as_written() {
        let found = imports(
            "import QtQuick 2.15\nimport App.Media as Media\nimport \"parts\"\nimport \"util.js\" as U\nItem {}\n",
        )
        .unwrap();
        assert_eq!(
            found,
            [
                Import::Module {
                    uri: "QtQuick".into(),
                    alias: None
                },
                Import::Module {
                    uri: "App.Media".into(),
                    alias: Some("Media".into())
                },
                Import::Path {
                    path: "parts".into(),
                    alias: None
                },
                Import::Path {
                    path: "util.js".into(),
                    alias: Some("U".into())
                },
            ]
        );
        assert_eq!(imports("Item {"), None);
    }
}
