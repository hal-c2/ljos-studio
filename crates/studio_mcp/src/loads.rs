//! What a write would stop loading, asked before it is written. A write that
//! goes well and leaves a file that won't load tells an agent the opposite of
//! what happened.
//!
//! Only what qml6 was seen to refuse is refused: blocking a valid write is
//! worse than missing a bad one, which the preview's load error still says.
//!
//! So every caller of `Names::type_is` reads its answer the same way. `Missing`
//! alone refuses a write. `Known` alone says a name is taken, or that nothing
//! has to come along for it. `Unsure` is neither: the write goes through, with
//! whatever the text's source had that might name it, and nothing is told the
//! name is free.

use std::sync::Mutex;

use ljos_mcp_core::SurfaceError;
use studio_core::outline::Import;
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
    let Some(names) = Names::of(studio.session.store(), doc, &source, &[]) else {
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
    imports: &[String],
) -> Result<(), SurfaceError> {
    let mut studio = lock(studio);
    let source = studio.session.text(doc).map_err(error)?.0.to_owned();
    check_text(studio.session.store(), doc, &source, text, imports)
}

/// The objects `text` writes, in `doc` once it imports `imports`.
pub(crate) fn check_text(
    store: &dyn Store,
    doc: &str,
    source: &str,
    text: &str,
    imports: &[String],
) -> Result<(), SurfaceError> {
    let (Some(names), Some(nodes)) = (
        Names::of(store, doc, source, imports),
        outline::outline(text),
    ) else {
        return Ok(());
    };
    for node in &nodes {
        if names.type_is(&node.ty, text) == TypeIs::Missing {
            return Err(unknown_type(doc, &node.ty));
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

/// What a paste of `text` from `from`, whose imports are `had`, has to import
/// into `doc`. A type not `Known` takes the first of them that makes it so; one
/// that stays `Unsure` might come from any, so all of them come.
pub(crate) fn imports_for(
    store: &dyn Store,
    doc: &str,
    source: &str,
    text: &str,
    from: &str,
    had: &[Import],
) -> Result<Vec<String>, SurfaceError> {
    let Some(nodes) = outline::outline(text) else {
        return Ok(Vec::new());
    };
    let is = |ty: &str, imports: &[String]| {
        Names::of(store, doc, source, imports).map(|names| names.type_is(ty, text))
    };
    let here = dir_of(doc);
    let there = dir_of(from);
    let target = outline::imports(source).unwrap_or_default();
    let offered: Vec<String> = had
        .iter()
        .filter_map(|import| match import {
            Import::Module { uri, alias: None } => Some(uri.clone()),
            Import::Path { path, alias: None } if !path.ends_with(".js") => {
                folder_from(there, path, here)
            }
            _ => None,
        })
        .filter(|offer| !imported_by(&target, here, offer))
        .collect();
    let mut wanted: Vec<String> = Vec::new();
    for node in &nodes {
        // What the text's qualifier meant stays behind with the alias.
        let alias_left = node.ty.rsplit_once('.').is_some_and(|(qualifier, _)| {
            let says = |import: &Import| match import {
                Import::Module { alias, .. } | Import::Path { alias, .. } => {
                    alias.as_deref() == Some(qualifier)
                }
            };
            had.iter().any(says) && !target.iter().any(says)
        });
        if alias_left {
            return Err(unknown_type(doc, &node.ty));
        }
        if is(&node.ty, &wanted) == Some(TypeIs::Known) {
            continue;
        }
        // The folder of the file it came from is imported without a word.
        let beside = store
            .exists(&in_dir(there, &format!("{}.qml", node.ty)))
            .then(|| folder_from(there, ".", here))
            .flatten();
        if let Some(folder) = beside.filter(|folder| !wanted.contains(folder)) {
            wanted.push(folder);
            continue;
        }
        let with = |uri: &String| [wanted.as_slice(), std::slice::from_ref(uri)].concat();
        if let Some(uri) = offered
            .iter()
            .find(|uri| is(&node.ty, &with(uri)) == Some(TypeIs::Known))
        {
            wanted.push(uri.clone());
        } else if is(&node.ty, &offered) == Some(TypeIs::Unsure) {
            for uri in &offered {
                if !wanted.contains(uri) {
                    wanted.push(uri.clone());
                }
            }
        }
    }
    check_text(store, doc, source, text, &wanted).map(|()| wanted)
}

fn dir_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn in_dir(dir: &str, file: &str) -> String {
    match dir {
        "" => file.to_owned(),
        _ => format!("{dir}/{file}"),
    }
}

/// How a file in `here` imports the folder `path` of a file in `there`; `None`
/// for its own folder, which needs no import, or one outside the project. A
/// `/` is what makes it a path and not a module.
fn folder_from(there: &str, path: &str, here: &str) -> Option<String> {
    let folder = crate::blocks::join(there, path)?;
    if folder == here {
        return None;
    }
    let to: Vec<&str> = folder.split('/').filter(|p| !p.is_empty()).collect();
    let at: Vec<&str> = here.split('/').filter(|p| !p.is_empty()).collect();
    let same = to.iter().zip(&at).take_while(|(a, b)| a == b).count();
    let mut steps = vec![".."; at.len() - same];
    steps.extend(&to[same..]);
    let relative = steps.join("/");
    Some(match relative.contains('/') {
        true => relative,
        false => format!("./{relative}"),
    })
}

fn imported_by(imports: &[Import], here: &str, offer: &str) -> bool {
    imports.iter().any(|import| match import {
        Import::Module { uri, alias: None } => uri == offer,
        Import::Path { path, alias: None } => {
            crate::blocks::join(here, path) == crate::blocks::join(here, offer)
        }
        _ => false,
    })
}

fn unknown_type(doc: &str, ty: &str) -> SurfaceError {
    refused(
        "unknown_type",
        format!("nothing `{doc}` imports has a type `{ty}`"),
    )
}

fn no_member(ty: &str, name: &str) -> SurfaceError {
    refused("no_such_member", format!("a `{ty}` has no `{name}`"))
}
