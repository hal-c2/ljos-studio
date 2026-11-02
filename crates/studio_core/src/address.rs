//! Addresses of objects that survive edits.
//!
//! A child-index path (`root/0/2`) is only true of one version of a file:
//! insert a sibling above and it names another object. A person selecting
//! an object, or an agent that listed a tree and edits it three calls
//! later, means *that object*. So the session hands out handles (`n7`) and
//! keeps each one pointing at its object:
//!
//! - after an edit Studio made itself, by the edit's own effect on paths
//!   (an insert shifts the siblings after it, a move carries the subtree);
//! - after any other change of the text (the code view, a file changed on
//!   disk), by the object's key: its `id`, else its literal `objectName`,
//!   else its type and place among unnamed siblings of that type, the rule
//!   hot reload uses to keep an object's state ([`ljos_ir::ObjectKey`]).
//!
//! A handle whose object is gone says so rather than quietly naming a
//! neighbour.

use ljos_ir::KeySeg;
use qml_edit::{NodePath, Step};
use serde::{Deserialize, Serialize};

use crate::outline::Node;

/// How a caller names an object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Address {
    /// A handle the session gave out (`n7`); stable for the session.
    Node(String),
    /// The object's `id` in the document.
    Id(String),
    /// A path as `document_tree` prints it (`root/0/contentItem:`), true of
    /// the current text only.
    Path(String),
}

// Written out: a dozen tools take an address and a `oneOf` of three objects
// costs each of them a kilobyte of the list.
impl schemars::JsonSchema for Address {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Address".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "One of `node` (session handle), `id`, `path` (from `document_tree`; this text only).",
            "type": "object",
            "additionalProperties": {"type": "string"},
            "minProperties": 1,
            "maxProperties": 1
        })
    }
}

/// What an edit Studio made did to paths.
#[derive(Clone, Debug)]
pub enum PathChange {
    /// Paths did not move (a binding changed).
    Same,
    Insert {
        parent: NodePath,
        index: usize,
    },
    Remove {
        object: NodePath,
    },
    Move {
        object: NodePath,
        parent: NodePath,
        index: usize,
    },
}

/// One handle's object.
#[derive(Clone, Debug)]
pub(crate) struct Tracked {
    pub handle: u64,
    /// `None` once the object is gone.
    pub path: Option<NodePath>,
    pub key: Vec<KeySeg>,
    pub ty: String,
    /// Its bytes in the current text, carried through every change of the
    /// text, also ones that leave it not parsing; `None` once a change
    /// touched its start or crossed its edges.
    pub span: Option<(usize, usize)>,
    /// Where it starts, while a change cut only its end off the span (a
    /// whole text written over it, its `id` renamed): enough to know the
    /// object by its place when its key no longer names it.
    pub head: Option<usize>,
}

impl Tracked {
    /// Moves [`span`](Self::span) over a change of the text.
    pub fn shift(&mut self, change: &qml_edit::Change) {
        let delta = change.replacement.len() as isize - change.range.len() as isize;
        let moved = |at: usize| at.saturating_add_signed(delta);
        // An object's head holds while changes land after it.
        self.head = self.head.and_then(|head| {
            if change.range.end <= head {
                Some(moved(head))
            } else if change.range.start > head {
                Some(head)
            } else {
                None
            }
        });
        let Some((start, end)) = self.span else {
            return;
        };
        self.span = if change.range.end <= start {
            Some((moved(start), moved(end)))
        } else if change.range.start >= end {
            Some((start, end))
        } else if change.range.start > start && change.range.end < end {
            // Inside it, past its type name and before its `}`.
            Some((start, moved(end)))
        } else {
            if change.range.start > start {
                self.head = Some(start);
            }
            None
        };
    }
}

/// Parses a path as [`NodePath`]'s `Display` writes it.
pub fn parse_path(text: &str) -> Option<NodePath> {
    let mut parts = text.split('/');
    if parts.next()? != "root" {
        return None;
    }
    let mut steps = Vec::new();
    for part in parts {
        let step = if let Some(name) = part.strip_suffix(':') {
            Step::Value(name.to_owned())
        } else if let Some(name) = part.strip_prefix("component ") {
            Step::Component(name.to_owned())
        } else {
            Step::Child(part.parse().ok()?)
        };
        steps.push(step);
    }
    Some(NodePath(steps))
}

fn child_at(path: &NodePath, prefix: &NodePath) -> Option<usize> {
    if path.0.len() > prefix.0.len() && path.0.starts_with(&prefix.0) {
        match path.0[prefix.0.len()] {
            Step::Child(n) => Some(n),
            _ => None,
        }
    } else {
        None
    }
}

fn with_child(path: &NodePath, at: usize, n: usize) -> NodePath {
    let mut steps = path.0.clone();
    steps[at] = Step::Child(n);
    NodePath(steps)
}

fn after_insert(path: &NodePath, parent: &NodePath, index: usize) -> NodePath {
    match child_at(path, parent) {
        Some(n) if n >= index => with_child(path, parent.0.len(), n + 1),
        _ => path.clone(),
    }
}

fn after_remove(path: &NodePath, object: &NodePath) -> Option<NodePath> {
    if path.0.starts_with(&object.0) {
        return None;
    }
    let parent = NodePath(object.0[..object.0.len() - 1].to_vec());
    let Some(Step::Child(removed)) = object.0.last() else {
        return Some(path.clone());
    };
    Some(match child_at(path, &parent) {
        Some(n) if n > *removed => with_child(path, parent.0.len(), n - 1),
        _ => path.clone(),
    })
}

impl PathChange {
    /// Where an object that was at `path` is after the change.
    pub fn apply(&self, path: &NodePath) -> Option<NodePath> {
        match self {
            PathChange::Same => Some(path.clone()),
            PathChange::Insert { parent, index } => Some(after_insert(path, parent, *index)),
            PathChange::Remove { object } => after_remove(path, object),
            PathChange::Move {
                object,
                parent,
                index,
            } => {
                // The destination is named in the text before the move; with
                // the object taken out it may have shifted.
                let parent = after_remove(parent, object)?;
                if path.0.starts_with(&object.0) {
                    let mut steps = parent.0.clone();
                    steps.push(Step::Child(*index));
                    steps.extend_from_slice(&path.0[object.0.len()..]);
                    return Some(NodePath(steps));
                }
                let out = after_remove(path, object)?;
                Some(after_insert(&out, &parent, *index))
            }
        }
    }
}

/// The node of `nodes` that `key` names: from its last `id` (ids are unique
/// in a file) or, without one, the whole chain from the root.
pub(crate) fn find_by_key<'n>(nodes: &'n [Node], key: &[KeySeg]) -> Option<&'n Node> {
    let anchor = key.iter().rposition(|s| matches!(s, KeySeg::Id(_)));
    match anchor {
        Some(i) => nodes.iter().find(|n| {
            n.key.len() >= key.len() - i && n.key[n.key.len() - (key.len() - i)..] == key[i..]
        }),
        None => nodes.iter().find(|n| n.key == key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> NodePath {
        parse_path(text).unwrap()
    }

    #[test]
    fn paths_parse_as_they_print() {
        for text in [
            "root",
            "root/0/2",
            "root/contentItem:/1",
            "root/component Row/0",
        ] {
            assert_eq!(p(text).to_string(), text);
        }
        assert!(parse_path("item/0").is_none());
        assert!(parse_path("root/x").is_none());
    }

    #[test]
    fn an_insert_shifts_later_siblings_and_their_children() {
        let change = PathChange::Insert {
            parent: p("root"),
            index: 1,
        };
        assert_eq!(change.apply(&p("root/0/3")), Some(p("root/0/3")));
        assert_eq!(change.apply(&p("root/1/3")), Some(p("root/2/3")));
        assert_eq!(change.apply(&p("root")), Some(p("root")));
    }

    #[test]
    fn a_removal_loses_the_subtree() {
        let change = PathChange::Remove {
            object: p("root/1"),
        };
        assert_eq!(change.apply(&p("root/1/0")), None);
        assert_eq!(change.apply(&p("root/2")), Some(p("root/1")));
        assert_eq!(change.apply(&p("root/0")), Some(p("root/0")));
    }

    #[test]
    fn a_move_carries_the_subtree() {
        // root/0 moves into root/2 (which becomes root/1) as its first child.
        let change = PathChange::Move {
            object: p("root/0"),
            parent: p("root/2"),
            index: 0,
        };
        assert_eq!(change.apply(&p("root/0/4")), Some(p("root/1/0/4")));
        assert_eq!(change.apply(&p("root/2/0")), Some(p("root/1/1")));
        assert_eq!(change.apply(&p("root/1")), Some(p("root/0")));
    }

    #[test]
    fn a_reorder_among_siblings() {
        let change = PathChange::Move {
            object: p("root/0"),
            parent: p("root"),
            index: 2,
        };
        assert_eq!(change.apply(&p("root/0")), Some(p("root/2")));
        assert_eq!(change.apply(&p("root/1")), Some(p("root/0")));
        assert_eq!(change.apply(&p("root/2")), Some(p("root/1")));
        assert_eq!(change.apply(&p("root/3")), Some(p("root/3")));
    }
}
