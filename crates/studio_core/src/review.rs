//! Design review comments, kept in Studio's sidecar (`.studio/review.json`)
//! and never in the QML: deleting the sidecar loses the comments and
//! nothing of the design (DESIGN.md goal 1).
//!
//! A comment is on an object. In a session it follows its object as a
//! handle does, through anybody's edits; on disk it is written with the
//! object's key, the name hot reload keeps state by, so the next session
//! finds the object again. A comment whose object is taken out is kept,
//! orphaned, with the place the object last had: a review of a design
//! that changed under it still says what it was about.

use std::collections::BTreeMap;

use ljos_ir::KeySeg;
use serde::{Deserialize, Serialize};

/// Where the comments are kept, relative to the project.
pub const FILE: &str = ".studio/review.json";

const VERSION: u32 = 1;

/// One comment and its replies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// `c1`, `c2`, …: unique in the project, never reused.
    pub id: String,
    pub doc: String,
    pub anchor: Anchor,
    /// Its object is gone; the anchor is where it last was.
    #[serde(default)]
    pub orphaned: bool,
    #[serde(default)]
    pub resolved: bool,
    /// The comment first, then its replies, oldest first.
    pub thread: Vec<Note>,
}

/// The object a comment is on, as it was when the comment was last
/// written down.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub key: Vec<KeySeg>,
    /// As `document_tree` prints it.
    pub path: String,
    pub line: usize,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub text: String,
    /// Milliseconds since the Unix epoch.
    pub at: u64,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    comments: Vec<Comment>,
}

/// The comments of a project, with the handles they follow this session.
#[derive(Debug, Default)]
pub(crate) struct Review {
    pub comments: Vec<Comment>,
    /// Comment id → the handle of its object, once looked up.
    pub bound: BTreeMap<String, u64>,
    /// Documents whose comments were looked up by key.
    pub looked_up: Vec<String>,
    /// Why the sidecar's comments could not be read: writing then would
    /// lose them, so it waits until somebody mends or deletes the file.
    pub unreadable: Option<String>,
    /// The sidecar as it was last read or written, `None` for none: a
    /// sidecar that is not this any more was changed by somebody else (a
    /// pull, a second Studio, a person deleting it), and is read again
    /// rather than written over.
    pub seen: Option<String>,
}

impl Review {
    /// The sidecar's comments; none when there is no sidecar, or one this
    /// version cannot read, which is then left alone.
    pub fn read(text: Option<&str>) -> Self {
        let mut review = Self {
            seen: text.map(str::to_owned),
            ..Self::default()
        };
        if let Some(text) = text {
            match serde_json::from_str::<OnDisk>(text) {
                // Its fields may mean more than this version knows, and
                // writing it as this version would drop them.
                Ok(disk) if disk.version > VERSION => {
                    review.unreadable = Some(format!(
                        "a later Studio wrote it (version {}; this one reads {VERSION})",
                        disk.version
                    ));
                }
                Ok(disk) => review.comments = disk.comments,
                Err(e) => review.unreadable = Some(e.to_string()),
            }
        }
        review
    }

    /// A sidecar that is there but could not be read at all.
    pub fn unreadable(why: String) -> Self {
        Self {
            unreadable: Some(why),
            ..Self::default()
        }
    }

    pub fn to_text(&self) -> String {
        let disk = OnDisk {
            version: VERSION,
            comments: self.comments.clone(),
        };
        let mut text = serde_json::to_string_pretty(&disk).expect("comments are JSON");
        text.push('\n');
        text
    }

    pub fn next_id(&self) -> String {
        let n = self
            .comments
            .iter()
            .filter_map(|c| c.id.strip_prefix('c')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("c{}", n + 1)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Comment> {
        self.comments.iter_mut().find(|c| c.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_read_back_as_written() {
        let mut review = Review::default();
        review.comments.push(Comment {
            id: review.next_id(),
            doc: "Main.qml".to_owned(),
            anchor: Anchor {
                key: vec![KeySeg::Id("title".to_owned())],
                path: "root/0".to_owned(),
                line: 5,
                ty: "Text".to_owned(),
                id: Some("title".to_owned()),
            },
            orphaned: false,
            resolved: false,
            thread: vec![Note {
                author: None,
                text: "Too small".to_owned(),
                at: 1,
            }],
        });
        let again = Review::read(Some(&review.to_text()));
        assert_eq!(again.comments, review.comments);
        assert_eq!(again.next_id(), "c2");
    }

    #[test]
    fn a_sidecar_that_does_not_read_has_no_comments() {
        let review = Review::read(Some("{"));
        assert!(review.comments.is_empty());
        assert!(review.unreadable.is_some());
        assert!(Review::read(None).comments.is_empty());
    }

    #[test]
    fn a_sidecar_a_later_version_wrote_is_left_alone() {
        let review = Review::read(Some(r#"{"version": 2, "comments": []}"#));
        assert!(
            review
                .unreadable
                .is_some_and(|why| why.contains("version 2"))
        );
    }
}
