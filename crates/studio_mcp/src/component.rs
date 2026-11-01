//! An object made into a component of the project: its text goes to a file
//! beside the document, an instance stays with its id and its place. Nothing
//! that worked may stop working, so a use across the new boundary becomes an
//! alias, or the whole thing is refused, naming it.

use std::ops::Range;

use ljos_mcp_core::SurfaceError;
use studio_core::place::{Word, code_words};
use studio_core::{Address, Error, Node, Step, outline};

use crate::layout_tools::fresh_id;
use crate::place_tools::refused;
use crate::tools::EditOut;
use crate::{Studio, error};

pub(crate) struct Extracted {
    pub(crate) edit: EditOut,
    pub(crate) component: String,
    pub(crate) aliases: Vec<String>,
}

fn check_name(name: &str) -> Result<(), SurfaceError> {
    let mut chars = name.chars();
    let typed = chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric());
    if typed {
        Ok(())
    } else {
        Err(SurfaceError::invalid_argument(format!(
            "`{name}` is not a type's name: a capital letter, then letters, digits or _"
        )))
    }
}

fn line_of(text: &str, at: usize) -> usize {
    text[..at].matches('\n').count() + 1
}

fn inside(range: &Range<usize>, at: usize) -> bool {
    range.start <= at && at < range.end
}

/// What it reads that would stay behind in the document.
fn check_reads_in(
    text: &str,
    node: &Node,
    tree: &[Node],
    kept: &[Range<usize>],
) -> Result<(), SurfaceError> {
    let within = |n: &&Node| n.start >= node.start && n.end <= node.end;
    let subtree: Vec<&Node> = tree.iter().filter(within).collect();
    let outer: Vec<&Node> = tree.iter().filter(|n| !within(n)).collect();
    let root = tree.first();
    let own = |name: &str| {
        subtree.iter().any(|n| {
            n.properties.iter().any(|p| p == name)
                || n.functions.iter().any(|f| f == name)
                || n.signals.iter().any(|s| s.name == name)
        })
    };
    let of_root = |name: &str| {
        root.is_some_and(|r| {
            r.properties.iter().any(|p| p == name)
                || r.functions.iter().any(|f| f == name)
                || r.signals.iter().any(|s| s.name == name)
        }) && !own(name)
    };
    let inline = |name: &str| {
        tree.iter().any(|n| {
            n.path
                .0
                .iter()
                .any(|s| matches!(s, Step::Component(c) if c == name))
        })
    };
    let body = &text[node.start..node.end];
    let words = code_words(body);
    for (i, word) in words.iter().enumerate() {
        let at = node.start + word.start;
        if word.member || kept.iter().any(|k| inside(k, at)) {
            continue;
        }
        let name = &body[word.start..word.end];
        let holder = subtree
            .iter()
            .filter(|n| n.start <= at && at < n.end)
            .max_by_key(|n| n.start)
            .copied();
        let leaves_through_parent = name == "parent"
            && (holder.is_some_and(|h| h.start == node.start)
                || words
                    .get(i + 1)
                    .is_some_and(|w| w.member && &body[w.start..w.end] == "parent"));
        if leaves_through_parent
            || outer.iter().any(|n| n.id.as_deref() == Some(name))
            || of_root(name)
            || inline(name)
        {
            return Err(refused(
                "reference_in",
                format!(
                    "line {} reads `{name}`, which stays in the document: pass it in as a \
                     property first",
                    line_of(text, at)
                ),
            ));
        }
    }
    Ok(())
}

struct Outward {
    /// `(name, inner id, property)` in the order first read.
    aliases: Vec<(String, String, String)>,
    /// What to rewrite to `instance.name`, with the index of its alias.
    rewrites: Vec<(Range<usize>, usize)>,
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

/// The uses of its inner ids from outside. A property read through one
/// becomes an alias; the rest can't cross, because nothing outside can hold
/// the inner object itself.
fn uses_from_outside(
    text: &str,
    node: &Node,
    tree: &[Node],
    kept: &[Range<usize>],
) -> Result<Outward, SurfaceError> {
    let inner: Vec<&str> = tree
        .iter()
        .filter(|n| n.start > node.start && n.end <= node.end)
        .filter_map(|n| n.id.as_deref())
        .collect();
    let mut taken: Vec<String> = node
        .properties
        .iter()
        .cloned()
        .chain(node.bindings.iter().map(|b| b.name.clone()))
        .collect();
    let mut out = Outward {
        aliases: Vec::new(),
        rewrites: Vec::new(),
    };
    let words: Vec<Word> = code_words(text);
    for (i, word) in words.iter().enumerate() {
        let name = &text[word.start..word.end];
        let moved = inside(&(node.start..node.end), word.start);
        let stays = kept.iter().any(|k| inside(k, word.start));
        if word.member || (moved && !stays) || !inner.contains(&name) {
            continue;
        }
        let no = |what: &str| {
            refused(
                "reference_out",
                format!(
                    "line {} {what} `{name}`, which would be inside the new file",
                    line_of(text, word.start)
                ),
            )
        };
        if stays {
            return Err(no("places the object by"));
        }
        let property = text[word.end..]
            .trim_start()
            .starts_with('.')
            .then(|| words.get(i + 1))
            .flatten()
            .filter(|w| w.member);
        let Some(property) = property else {
            return Err(no("uses"));
        };
        let what = &text[property.start..property.end];
        if text[property.end..].trim_start().starts_with(['(', '.']) {
            return Err(no(&format!("calls or goes through `.{what}` of")));
        }
        let known = out
            .aliases
            .iter()
            .position(|(_, id, p)| id == name && p == what);
        let index = known.unwrap_or_else(|| {
            let stem = format!("{name}{}", capitalised(what));
            let alias = fresh_id(&stem, &taken);
            taken.push(alias.clone());
            out.aliases.push((alias, name.to_owned(), what.to_owned()));
            out.aliases.len() - 1
        });
        out.rewrites.push((word.start..property.end, index));
    }
    Ok(out)
}

/// The range to take away with a member: the `;` after it and, when it had
/// its line alone, the line.
fn member_with_line(text: &str, member: &Range<usize>) -> Range<usize> {
    let mut end = member.end;
    let rest = &text[end..];
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if rest[spaces..].starts_with(';') {
        end += spaces + 1;
    }
    let rest = &text[end..];
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let line_start = text[..member.start].rfind('\n').map_or(0, |n| n + 1);
    if text[line_start..member.start].trim().is_empty() && rest[spaces..].starts_with('\n') {
        return line_start..end + spaces + 1;
    }
    member.start..end + spaces
}

/// Where in `text` the newlines inside strings are: the spaces after those
/// are the string's value.
fn newlines_in_strings(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = text[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |n| i + 2 + n + 2);
            }
            quote @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\n' {
                        found.push(i);
                    }
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    found
}

fn dedented(text: &str, indent: &str) -> String {
    let held = newlines_in_strings(text);
    let mut out = String::with_capacity(text.len());
    let mut after = None;
    for line in text.split_inclusive('\n') {
        let start = line.as_ptr() as usize - text.as_ptr() as usize;
        let in_string = after.is_some_and(|newline| held.contains(&newline));
        out.push_str(if in_string {
            line
        } else {
            line.strip_prefix(indent).unwrap_or(line)
        });
        after = Some(start + line.len() - 1);
    }
    out
}

fn splice(text: &str, mut changes: Vec<(Range<usize>, String)>) -> String {
    changes.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut out = text.to_owned();
    for (range, with) in changes {
        out.replace_range(range, &with);
    }
    out
}

fn indent_at(text: &str, at: usize) -> &str {
    let start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let line = &text[start..at];
    &line[..line.len() - line.trim_start().len()]
}

/// The new file: what the document imports, then the object without its place.
fn component_text(
    text: &str,
    node: &Node,
    kept: &[Range<usize>],
    outward: &Outward,
    root: &Node,
) -> String {
    let header: String = text[..root.start]
        .lines()
        .filter(|l| l.starts_with("import ") || l.starts_with("pragma "))
        .map(|l| format!("{l}\n"))
        .collect();
    let body = &text[node.start..node.end];
    let mut changes: Vec<(Range<usize>, String)> = kept
        .iter()
        .map(|k| {
            let whole = member_with_line(text, k);
            (
                whole.start - node.start..whole.end - node.start,
                String::new(),
            )
        })
        .collect();
    let indent = indent_at(text, node.start);
    if !outward.aliases.is_empty() {
        let declared: String = outward
            .aliases
            .iter()
            .map(|(name, id, property)| {
                format!("\n{indent}    property alias {name}: {id}.{property}")
            })
            .collect();
        changes.push(alias_insertion(text, node, declared, indent, &changes));
    }
    let object = splice(body, changes);
    let object = dedented(&object, indent);
    if header.is_empty() {
        format!("{object}\n")
    } else {
        format!("{header}\n{object}\n")
    }
}

/// Where the aliases go, as a change to the object's text: after the id,
/// which a reader looks for first, or else after the brace. What is left of
/// that line stays with what it belongs to: a comment before the aliases, and
/// members, on a line of their own, after them.
fn alias_insertion(
    text: &str,
    node: &Node,
    declared: String,
    indent: &str,
    removed: &[(Range<usize>, String)],
) -> (Range<usize>, String) {
    let body = &text[node.start..node.end];
    let blanks = |at: usize| {
        let rest = &body[at..];
        at + rest.len() - rest.trim_start_matches([' ', '\t']).len()
    };
    let at = outline::id_member(text, node.start).map_or_else(
        || body.find('{').map_or(body.len(), |n| n + 1),
        |id| {
            let end = id.end - node.start;
            let past = blanks(end);
            if body[past..].starts_with(';') {
                past + 1
            } else {
                end
            }
        },
    );
    // A member going to the instance is no longer on this line.
    let mut next = blanks(at);
    while let Some((range, _)) = removed
        .iter()
        .find(|(r, _)| r.start == next && r.end > next)
    {
        next = blanks(range.end);
    }
    let rest = &body[next..];
    if rest.starts_with('\n') || rest.is_empty() {
        (at..at, declared)
    } else if rest.starts_with("//") {
        let end = next + rest.find('\n').unwrap_or(rest.len());
        (end..end, declared)
    } else {
        let then = if rest.starts_with('}') { "" } else { "    " };
        (at..blanks(at), format!("{declared}\n{indent}{then}"))
    }
}

fn instance_text(
    text: &str,
    node: &Node,
    name: &str,
    id: Option<&str>,
    kept: &[Range<usize>],
) -> String {
    let members: Vec<String> = id
        .map(|id| format!("id: {id}"))
        .into_iter()
        .chain(
            kept.iter()
                .map(|k| text[k.clone()].trim_end_matches([';', ' ']).to_owned()),
        )
        .collect();
    if members.is_empty() {
        format!("{name} {{}}")
    } else if text[node.start..node.end].contains('\n') {
        let indent = indent_at(text, node.start);
        let lines: String = members
            .iter()
            .map(|m| format!("{indent}    {m}\n"))
            .collect();
        format!("{name} {{\n{lines}{indent}}}")
    } else {
        format!("{name} {{ {} }}", members.join("; "))
    }
}

pub(crate) fn extract(
    studio: &mut Studio,
    doc: &str,
    address: &Address,
    name: &str,
    generation: Option<u64>,
    dry_run: bool,
) -> Result<Extracted, SurfaceError> {
    check_name(name)?;
    let (text, current) = studio.session.text(doc).map_err(error)?;
    let text = text.to_owned();
    if let Some(given) = generation.filter(|g| *g != current) {
        return Err(error(Error::Stale {
            doc: doc.to_owned(),
            given,
            current,
        }));
    }
    let node = studio
        .session
        .resolve(doc, address)
        .map_err(error)?
        .1
        .clone();
    if node.path.0.is_empty() {
        return Err(refused(
            "root",
            "the document's root is a component already",
        ));
    }
    if node.path.0.iter().any(|s| matches!(s, Step::Component(_))) {
        return Err(refused(
            "in_component",
            "it is inside an inline component, whose ids are its own: extract that first",
        ));
    }
    let component = match doc.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{name}.qml"),
        None => format!("{name}.qml"),
    };
    if studio.session.store().exists(&component) {
        return Err(refused("exists", format!("`{component}` is there already")));
    }
    let tree: Vec<Node> = studio
        .session
        .tree(doc)
        .map_err(error)?
        .0
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    let kept = outline::place_members(&text, node.start);
    check_reads_in(&text, &node, &tree, &kept)?;
    let outward = uses_from_outside(&text, &node, &tree, &kept)?;
    let taken: Vec<String> = tree.iter().filter_map(|n| n.id.clone()).collect();
    let id = node.id.clone().or_else(|| {
        (!outward.aliases.is_empty()).then(|| {
            let mut stem = name.to_owned();
            stem[..1].make_ascii_lowercase();
            fresh_id(&stem, &taken)
        })
    });
    let root = tree.first().unwrap_or(&node);
    let new_text = component_text(&text, &node, &kept, &outward, root);
    let mut changes: Vec<(Range<usize>, String)> = vec![(
        node.start..node.end,
        instance_text(&text, &node, name, id.as_deref(), &kept),
    )];
    changes.extend(outward.rewrites.iter().map(|(range, index)| {
        let instance = id.as_deref().unwrap_or_default();
        (
            range.clone(),
            format!("{instance}.{}", outward.aliases[*index].0),
        )
    }));
    let new_doc = splice(&text, changes);
    if outline::outline(&new_doc).is_none() || outline::outline(&new_text).is_none() {
        return Err(refused(
            "not_parsable",
            "what it would write does not parse: its text may be unusual, edit it first",
        ));
    }
    let outcome = studio
        .session
        .create_with_edit(
            &component,
            &new_text,
            doc,
            &new_doc,
            "component_extract",
            dry_run,
        )
        .map_err(error)?;
    Ok(Extracted {
        edit: outcome.into(),
        component,
        aliases: outward.aliases.into_iter().map(|(name, ..)| name).collect(),
    })
}
