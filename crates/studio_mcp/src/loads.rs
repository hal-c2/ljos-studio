//! What a write would stop loading, asked before it is written. A write that
//! goes well and leaves a file that won't load tells an agent the opposite of
//! what happened.
//!
//! Only what is certain is refused: a type or member Studio has no description
//! of may still be there, and only running the document could tell.

use std::sync::Mutex;

use ljos_mcp_core::SurfaceError;
use studio_core::{Node, Store, outline};

use crate::place_tools::refused;
use crate::signals::{Names, TypeIs};
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
    let written = says_member(&source[object.start..object.end], name);
    if !written && names.lacks(&object.ty, Some(object), name) {
        return Err(no_member(&object.ty, name));
    }
    Ok(())
}

/// Whether an object's text binds `name` already, with a value or an object,
/// in itself or in something inside it: a write there changes nothing about
/// whether the file loads.
fn says_member(object: &str, name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    let part = |c: char| c.is_alphanumeric() || c == '_';
    object.match_indices(base).any(|(at, _)| {
        let after = object[at + base.len()..].trim_start();
        !object[..at].chars().next_back().is_some_and(part)
            && (after.starts_with([':', '{']) || after.starts_with('.'))
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
                format!(
                    "nothing `{doc}` imports has a type `{}`, as far as Studio knows: \
                     if it is there, type it in the code view",
                    node.ty
                ),
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

/// Which of `modules` a paste of `text` into `doc` has to import, or the
/// refusal when one import won't do.
pub(crate) fn import_for(
    store: &dyn Store,
    doc: &str,
    source: &str,
    text: &str,
    modules: &[String],
) -> Result<Option<String>, SurfaceError> {
    if check_text(store, doc, source, text, None).is_ok() {
        return Ok(None);
    }
    let brings = modules
        .iter()
        .find(|uri| check_text(store, doc, source, text, Some(uri.as_str())).is_ok());
    match brings {
        Some(uri) => Ok(Some(uri.clone())),
        None => check_text(store, doc, source, text, None).map(|()| None),
    }
}

fn no_member(ty: &str, name: &str) -> SurfaceError {
    refused("no_such_member", format!("a `{ty}` has no `{name}`"))
}
