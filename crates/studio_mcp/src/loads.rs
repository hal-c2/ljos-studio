//! What a write would stop loading, asked before it is written. A write that
//! goes well and leaves a file that won't load tells an agent the opposite of
//! what happened.
//!
//! Only what qml6 was seen to refuse is refused: blocking a valid write is
//! worse than missing a bad one, which the preview's load error still says.

use std::sync::Mutex;

use ljos_mcp_core::SurfaceError;
use studio_core::{Node, Step, Store, outline};

use crate::place_tools::refused;
use crate::signals::{Names, TypeIs, not_an_id};
use crate::{Studio, error, lock};

/// `name` on `object` of `doc`, if `object`'s type is known not to have it.
pub(crate) fn check_member(
    studio: &mut Studio,
    doc: &str,
    object: &Node,
    name: &str,
) -> Result<(), SurfaceError> {
    let source = studio.session.text(doc).map_err(error)?.0.to_owned();
    let Some(names) = Names::of(studio.session.store(), doc, &source, None) else {
        return Ok(());
    };
    if !says_member(&source, object, name) && names.lacks(&object.ty, Some(object), name) {
        return Err(no_member(&object.ty, name));
    }
    Ok(())
}

/// Whether `object` itself binds `name` already, with a value or an object: a
/// write there changes nothing about whether the file loads.
fn says_member(source: &str, object: &Node, name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    let is_base = |bound: &str| bound.split('.').next() == Some(base);
    object.bindings.iter().any(|b| is_base(&b.name))
        || outline::outline(source).is_some_and(|nodes| {
            nodes.iter().any(|node| {
                let (last, parent) = match node.path.0.split_last() {
                    Some(split) => split,
                    None => return false,
                };
                parent == object.path.0.as_slice()
                    && matches!(last, Step::Value(bound) if is_base(bound))
            })
        })
}

pub(crate) fn check_insert(
    studio: &Mutex<Studio>,
    doc: &str,
    text: &str,
    import: Option<&str>,
) -> Result<(), SurfaceError> {
    let mut studio = lock(studio);
    let source = studio.session.text(doc).map_err(error)?.0.to_owned();
    check_text(studio.session.store(), doc, &source, text, import)
}

/// The objects `text` writes, in `doc` once it imports `import`.
pub(crate) fn check_text(
    store: &dyn Store,
    doc: &str,
    source: &str,
    text: &str,
    import: Option<&str>,
) -> Result<(), SurfaceError> {
    let (Some(names), Some(nodes)) = (
        Names::of(store, doc, source, import),
        outline::outline(text),
    ) else {
        return Ok(());
    };
    for node in &nodes {
        if names.type_is(&node.ty, text) == TypeIs::Missing {
            return Err(refused(
                "unknown_type",
                format!("nothing `{doc}` imports has a type `{}`", node.ty),
            ));
        }
        if let Some(id) = node.id.as_deref().filter(|id| not_an_id(id)) {
            return Err(refused(
                "not_an_id",
                format!("`{id}` is not an id: QML keeps the word for itself"),
            ));
        }
        if let Some(b) = node
            .bindings
            .iter()
            .find(|b| names.lacks(&node.ty, Some(node), &b.name))
        {
            return Err(no_member(&node.ty, &b.name));
        }
    }
    Ok(())
}

/// Which of `modules` a paste of `text` into `doc` has to import: the first
/// that names all of it, since a module not described might only seem to. No
/// such module leaves what the document can name as it is, or the refusal.
pub(crate) fn import_for(
    store: &dyn Store,
    doc: &str,
    source: &str,
    text: &str,
    modules: &[String],
) -> Result<Option<String>, SurfaceError> {
    let all_known = |import: Option<&str>| {
        let names = Names::of(store, doc, source, import);
        let nodes = outline::outline(text);
        names.zip(nodes).is_some_and(|(names, nodes)| {
            nodes
                .iter()
                .all(|node| names.type_is(&node.ty, text) == TypeIs::Known)
        })
    };
    if all_known(None) {
        return Ok(None);
    }
    if let Some(uri) = modules.iter().find(|uri| all_known(Some(uri))) {
        return Ok(Some(uri.clone()));
    }
    check_text(store, doc, source, text, None).map(|()| None)
}

fn no_member(ty: &str, name: &str) -> SurfaceError {
    refused("no_such_member", format!("a `{ty}` has no `{name}`"))
}
