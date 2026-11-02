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
use crate::signals::{Finds, Names, TypeIs, not_an_id};
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

/// What a paste of `texts`, together, from `from`, whose imports are `had`, has
/// to import into `doc`. A type not `Known` takes the first of them that makes it so; one
/// that stays `Unsure` might come from any, so all of them come. A type whose
/// place in `from` can be said stays there, by an import of it, or the paste is
/// refused. An import wins over a file beside the document, so one that would
/// turn an object `doc` has into something else stays out, and is refused if the
/// paste needs it.
pub(crate) fn imports_for(
    store: &dyn Store,
    doc: &str,
    source: &str,
    texts: &[&str],
    from: &str,
    had: &[Import],
) -> Result<Vec<String>, SurfaceError> {
    let nodes: Vec<_> = texts
        .iter()
        .filter_map(|text| outline::outline(text))
        .flatten()
        .collect();
    let beside = texts.join("\n");
    let is = |ty: &str, imports: &[String]| {
        Names::of(store, doc, source, imports).map(|names| names.type_is(ty, &beside))
    };
    let here = dir_of(doc);
    let there = dir_of(from);
    let target = outline::imports(source).unwrap_or_default();
    let offered: Vec<String> = had
        .iter()
        .filter_map(|import| match import {
            Import::Module { uri, alias: None } => Some(uri.clone()),
            Import::Path { path, alias: None } if !path.ends_with(".js") => {
                if path.starts_with('/') || path.contains(':') {
                    Some(path.clone())
                } else {
                    folder_from(there, path, here)
                }
            }
            _ => None,
        })
        .filter(|offer| !imported_by(&target, here, offer))
        .collect();
    let shadows =
        |offer: &str| Names::of(store, doc, source, &[]).and_then(|names| names.shadowed_by(offer));
    let needed = |offer: &str| match shadows(offer) {
        Some(ty) => Err(refused(
            "import_changes_type",
            format!("importing `{offer}` would make the `{ty}` in `{doc}` another type"),
        )),
        None => Ok(offer.to_owned()),
    };
    // The last folder imported wins, so what comes keeps the source's order; a
    // folder that came for the file beside the source had nothing to beat.
    let rank = |import: &String| {
        offered
            .iter()
            .position(|o| o == import)
            .map_or(0, |i| i + 1)
    };
    let with_offer = |wanted: &[String], offer: Option<&str>| {
        let mut with = wanted.to_vec();
        // An import the target has stays where it is: adding it moves nothing.
        with.extend(
            offer
                .filter(|o| !imported_by(&target, here, o))
                .map(str::to_owned)
                .filter(|o| !wanted.contains(o)),
        );
        with.sort_by_key(rank);
        with
    };
    let was_at = Names::at(store, from, had);
    // Where either file imports what can't be read, an answer here may be
    // wrong, so it can bring imports along but never refuses.
    let unread = was_at.bare_unread()
        || Names::of(store, doc, source, &[]).is_some_and(|names| names.bare_unread());
    let changes = |ty: &str| {
        refused(
            "import_changes_type",
            format!("`{ty}` in `{doc}` would be another type, and no import can keep it"),
        )
    };
    let bring_all = |wanted: &mut Vec<String>| {
        for uri in &offered {
            if shadows(uri).is_none() {
                *wanted = with_offer(wanted, Some(uri));
            }
        }
    };
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
        let Some(names) = Names::of(store, doc, source, &wanted) else {
            continue;
        };
        // Only an import of where it was can keep it that, whatever the target
        // would do with the name: another type, or none yet.
        if let Some(was) = was_at.finds(&node.ty) {
            let would = names.finds(&node.ty);
            if would.is_some_and(|would| was.same(&would)) {
                continue;
            }
            let offer = match &was {
                Finds::File(file) => folder_from("", dir_of(file), here),
                Finds::Modules(modules) => modules.first().cloned(),
            };
            let with = with_offer(&wanted, offer.as_deref());
            let kept = Names::of(store, doc, source, &with)
                .and_then(|names| names.finds(&node.ty))
                .is_some_and(|now| now.same(&was));
            match offer {
                Some(offer) if kept => {
                    needed(&offer)?;
                    wanted = with;
                }
                _ if unread => bring_all(&mut wanted),
                _ => return Err(changes(&node.ty)),
            }
            continue;
        }
        // Something the source imports that nothing here can read has it, so
        // a name the target has as well may be another type.
        if unread && !node.ty.contains('.') && names.finds(&node.ty).is_some() {
            bring_all(&mut wanted);
            continue;
        }
        if is(&node.ty, &wanted) == Some(TypeIs::Known) {
            continue;
        }
        // The folder of the file it came from is imported without a word. A
        // qualified type is in the file its first part names.
        let file = node.ty.split('.').next().unwrap_or(&node.ty);
        let beside = store
            .exists(&in_dir(there, &format!("{file}.qml")))
            .then(|| folder_from(there, ".", here))
            .flatten();
        if let Some(folder) = beside {
            needed(&folder)?;
            wanted = with_offer(&wanted, Some(&folder));
            continue;
        }
        let with = |uri: &String| [wanted.as_slice(), std::slice::from_ref(uri)].concat();
        let making: Vec<&String> = offered
            .iter()
            .filter(|uri| is(&node.ty, &with(uri)) == Some(TypeIs::Known))
            .collect();
        if let Some(uri) = making.first() {
            let free = making.iter().find(|uri| shadows(uri).is_none());
            let uri = free.unwrap_or(uri);
            needed(uri)?;
            wanted = with_offer(&wanted, Some(uri));
        } else if is(&node.ty, &offered) == Some(TypeIs::Unsure) {
            bring_all(&mut wanted);
        }
    }
    // A folder that came for one part may have taken another part's name.
    if !unread && let Some(names) = Names::of(store, doc, source, &wanted) {
        for node in &nodes {
            if let Some(was) = was_at.finds(&node.ty)
                && !names.finds(&node.ty).is_some_and(|now| now.same(&was))
            {
                return Err(changes(&node.ty));
            }
        }
    }
    for text in texts {
        check_text(store, doc, source, text, &wanted)?;
    }
    Ok(wanted)
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
