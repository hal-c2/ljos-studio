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

use std::collections::BTreeSet;
use std::sync::Mutex;

use ljos_mcp_core::SurfaceError;
use studio_core::modules::{Modules, member_uses};
use studio_core::outline::Import;
use studio_core::{Node, Step, Store, outline};

use crate::place_tools::refused;
use crate::signals::{Keeps, Lands, Names, TypeIs, Unvouched, compare, measured_in, not_an_id};
use crate::{Studio, error, lock, toolkit};

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

/// The toolkit imports that have to come along for `text` to go into `doc`:
/// for each type nothing there has, its module when that is the only one known
/// to export it and is one Studio offers it from. A type two modules export is
/// refused, so the agent says which. None come along when the file would still
/// not load, or a name it resolves would become another thing: the insert is
/// then refused as any other with a type nothing has.
pub(crate) fn toolkit_imports(
    studio: &Mutex<Studio>,
    doc: &str,
    text: &str,
    imports: &[String],
) -> Result<Vec<String>, SurfaceError> {
    let mut studio = lock(studio);
    let source = studio.session.text(doc).map_err(error)?.0.to_owned();
    let store = studio.session.store();
    let (Some(names), Some(nodes)) = (
        Names::of(store, doc, &source, imports),
        outline::outline(text),
    ) else {
        return Ok(Vec::new());
    };
    let mut missing: Vec<&str> = Vec::new();
    for node in &nodes {
        let ty = node.ty.as_str();
        if !ty.contains('.') && !missing.contains(&ty) && names.type_is(ty, text) == TypeIs::Missing
        {
            missing.push(ty);
        }
    }
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    let project = Modules::read(store);
    let target = outline::imports(&source).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for ty in missing {
        let offered: Vec<&str> = toolkit::ALWAYS
            .into_iter()
            .filter(|uri| {
                toolkit::names(uri).is_some_and(|types| types.iter().any(|name| name == ty))
            })
            .collect();
        let known: BTreeSet<&str> = offered
            .iter()
            .copied()
            .chain(measured_in(ty))
            .chain(project.exporters(ty))
            .collect();
        if known.len() > 1 {
            return Err(several_have(doc, ty, &known));
        }
        // With one known, an offered one is that one. An alias is the file's
        // choice of how the module's types are written.
        let Some(&uri) = offered.first() else {
            continue;
        };
        let imported = target
            .iter()
            .any(|import| matches!(import, Import::Module { uri: had, .. } if had == uri));
        if !imported && !out.iter().any(|had| had == uri) {
            out.push(uri.to_owned());
        }
    }
    if out.is_empty() {
        return Ok(out);
    }
    let Some(with) = Names::of(store, doc, &source, &[imports, &out].concat()) else {
        return Ok(Vec::new());
    };
    let held = outline::outline(&source).unwrap_or_default();
    let own = |name: &str| {
        outline::declares_component(&held, name) || outline::declares_component(&nodes, name)
    };
    let still_missing = held.iter().any(|node| {
        !node.ty.contains('.') && !own(&node.ty) && with.lands(&node.ty) == Lands::Nowhere
    });
    let moved = held
        .iter()
        .chain(&nodes)
        .map(|node| node.ty.as_str())
        .chain(member_uses(&source))
        .chain(member_uses(text))
        .filter(|name| !own(name))
        .any(|name| takes_over(&names.lands(name), &with.lands(name)));
    Ok(if still_missing || moved {
        Vec::new()
    } else {
        out
    })
}

/// Whether more imports made what landed at `was` something else. The
/// toolkit's modules share a type of one name; an app's module has its own.
fn takes_over(was: &Lands, now: &Lands) -> bool {
    was != now
        && match was {
            Lands::Folder(_) => true,
            Lands::Modules(modules) => modules
                .iter()
                .any(|uri| !toolkit::ALWAYS.contains(&uri.as_str())),
            Lands::Nowhere | Lands::CantTell => false,
        }
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
    // A file has one of each, however deep they are written.
    let here = outline::outline(source).unwrap_or_default();
    if let Some(twice) = outline::components(&nodes)
        .into_iter()
        .find(|name| outline::declares_component(&here, name))
    {
        return Err(duplicate_component(doc, twice));
    }
    // qml6 won't load a component and a `Head.Inner` of its name in one file,
    // whatever `Inner` was, and it does when the head is an alias. What the
    // file already says isn't held against a write.
    let ours = outline::components(&here);
    let brought = outline::components(&nodes);
    let imported = outline::imports(source).unwrap_or_default();
    let taken = |ty: &str, by: &[&[&str]]| {
        ty.split_once('.').is_some_and(|(head, _)| {
            by.iter().any(|names| names.contains(&head))
                && !imported.iter().any(|i| has_alias(i, head))
        })
    };
    if let Some(left) = nodes
        .iter()
        .find(|node| taken(&node.ty, &[&ours, &brought]))
        .or_else(|| here.iter().find(|node| taken(&node.ty, &[&brought])))
    {
        return Err(refused(
            "component_takes_name",
            format!(
                "an inline component takes the name `{}` in `{doc}`",
                left.ty
            ),
        ));
    }
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

/// What a paste brings and what it couldn't vouch for.
pub(crate) struct Pasted {
    pub(crate) imports: Vec<String>,
    pub(crate) unvouched: Vec<Unvouched>,
}

/// What a paste of `texts`, together, from `from`, whose imports are `had`, adds
/// to `doc`: the least of the source's imports, in its order, that a name not
/// landing where it did needs, leaving out any that certainly moves what `doc`
/// has. Whatever `lands` can't then show as the same, pasted or already there, is
/// said in the answer, not refused: the
/// resolver is sure of less than qml6, and only qml6's refusals refuse. An inline
/// component wins over every import in the whole file, so a name one takes,
/// declared by the paste or by `doc`, is said too.
pub(crate) fn imports_for(
    store: &dyn Store,
    doc: &str,
    source: &str,
    texts: &[&str],
    from: &str,
    had: &[Import],
) -> Result<Pasted, SurfaceError> {
    let pasted: Vec<Vec<Node>> = texts.iter().filter_map(|t| outline::outline(t)).collect();
    let mut brought: Vec<&str> = Vec::new();
    for name in pasted.iter().flat_map(|nodes| outline::components(nodes)) {
        if brought.contains(&name) {
            return Err(duplicate_component(doc, name));
        }
        brought.push(name);
    }
    let mut names: Vec<String> = Vec::new();
    for node in pasted.iter().flatten() {
        let inline = pasted
            .iter()
            .any(|nodes| outline::declares_component(nodes, &node.ty));
        if !names.contains(&node.ty) && !inline {
            names.push(node.ty.clone());
        }
    }
    let target = outline::imports(source).unwrap_or_default();
    // What a qualifier meant stays behind with the alias: qml6 won't load it.
    if let Some(left) = names.iter().find(|name| {
        name.rsplit_once('.').is_some_and(|(qualifier, _)| {
            had.iter().any(|i| has_alias(i, qualifier))
                && !target.iter().any(|i| has_alias(i, qualifier))
        })
    }) {
        return Err(unknown_type(doc, left));
    }
    let (Some(before), false) = (
        Names::of(store, doc, source, &[]),
        from == doc && had == target,
    ) else {
        check_all(store, doc, source, texts, &[])?;
        return Ok(Pasted {
            imports: Vec::new(),
            unvouched: Vec::new(),
        });
    };
    let held = outline::outline(source).unwrap_or_default();
    let was_at = Names::at(store, from, had);
    let here = dir_of(doc);
    let there = dir_of(from);
    let mut holds: Vec<String> = Vec::new();
    for node in &held {
        if !holds.contains(&node.ty) && !outline::declares_component(&held, &node.ty) {
            holds.push(node.ty.clone());
        }
    }
    let moves = |offer: &str| {
        Names::of(store, doc, source, &[offer.to_owned()]).is_some_and(|with| {
            holds
                .iter()
                .any(|ty| compare(&before.lands(ty), &with.lands(ty)) == Keeps::Different)
        })
    };
    // The folder of the file it came from first: it is imported without a word,
    // and every folder the source names beat it there.
    let implicit = folder_from(there, ".", here);
    let mut offers: Vec<String> = Vec::new();
    for offer in implicit
        .iter()
        .cloned()
        .chain(had.iter().filter_map(|import| match import {
            Import::Module { uri, alias: None } => Some(uri.clone()),
            Import::Path { path, alias: None } if !path.ends_with(".js") => {
                if path.starts_with('/') || path.contains(':') {
                    Some(path.clone())
                } else {
                    folder_from(there, path, here)
                }
            }
            _ => None,
        }))
    {
        if !offers.contains(&offer) && !imported_by(&target, here, &offer) && !moves(&offer) {
            offers.push(offer);
        }
    }
    let mut wanted: Vec<&String> = Vec::new();
    for name in &names {
        let was = was_at.lands(name);
        if compare(&was, &before.lands(name)) == Keeps::Same
            || outline::declares_component(&held, name)
        {
            continue;
        }
        match &was {
            Lands::Folder(dir) => {
                let folder = folder_from("", dir, here);
                wanted.extend(offers.iter().filter(|o| Some(*o) == folder.as_ref()));
            }
            Lands::Modules(modules) => {
                wanted.extend(offers.iter().find(|o| modules.contains(o)));
            }
            Lands::CantTell => {
                let file = name.split('.').next().unwrap_or(name);
                let beside_it = store.exists(&in_dir(there, &format!("{file}.qml")));
                wanted.extend(
                    offers
                        .iter()
                        .filter(|o| beside_it || Some(*o) != implicit.as_ref()),
                );
            }
            Lands::Nowhere => {}
        }
    }
    let imports: Vec<String> = offers
        .iter()
        .filter(|offer| wanted.contains(offer))
        .cloned()
        .collect();
    check_all(store, doc, source, texts, &imports)?;
    let mut unvouched = Vec::new();
    if let Some(after) = Names::of(store, doc, source, &imports) {
        for name in &names {
            let (was, now) = (was_at.lands(name), after.lands(name));
            let taken = outline::declares_component(&held, name);
            if taken || compare(&was, &now) != Keeps::Same {
                unvouched.push(Unvouched {
                    ty: name.clone(),
                    was: was.say(),
                    now: if taken {
                        IN_THIS_FILE.to_owned()
                    } else {
                        now.say()
                    },
                    held: false,
                });
            }
        }
        // An import that only may move what the file has goes in, since what
        // needs it may not load otherwise, and the answer says so. The same
        // answer twice says nothing the import caused, unless it can't be read.
        let unread_added = after.unread_imports() > before.unread_imports();
        for ty in &holds {
            let (was, now) = (before.lands(ty), after.lands(ty));
            let taken = brought.contains(&ty.as_str());
            let moved = !imports.is_empty()
                && compare(&was, &now) != Keeps::Same
                && (was != now || (was == Lands::CantTell && unread_added));
            if taken || moved {
                unvouched.push(Unvouched {
                    ty: ty.clone(),
                    was: was.say(),
                    now: if taken {
                        IN_THIS_FILE.to_owned()
                    } else {
                        now.say()
                    },
                    held: true,
                });
            }
        }
    }
    Ok(Pasted { imports, unvouched })
}

fn has_alias(import: &Import, name: &str) -> bool {
    match import {
        Import::Module { alias, .. } | Import::Path { alias, .. } => alias.as_deref() == Some(name),
    }
}

fn check_all(
    store: &dyn Store,
    doc: &str,
    source: &str,
    texts: &[&str],
    imports: &[String],
) -> Result<(), SurfaceError> {
    texts
        .iter()
        .try_for_each(|text| check_text(store, doc, source, text, imports))
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

/// Where a name an inline component takes lands, in an answer.
const IN_THIS_FILE: &str = "an inline component in this file";

pub(crate) fn duplicate_component(doc: &str, name: &str) -> SurfaceError {
    refused(
        "duplicate_component",
        format!("`{doc}` already declares a `component {name}`: qml6 won't load it twice"),
    )
}

fn several_have(doc: &str, ty: &str, modules: &BTreeSet<&str>) -> SurfaceError {
    let modules = modules.iter().copied().collect::<Vec<_>>().join(", ");
    refused(
        "unknown_type",
        format!(
            "nothing `{doc}` imports has a type `{ty}`, and more than one module does \
             ({modules}): name the one meant as `import`"
        ),
    )
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
