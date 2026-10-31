//! `review` (MCP.md): design review comments on a document's objects, kept
//! in Studio's sidecar. Reading them is inspecting; writing them changes no
//! document, but it is a change all the same, so it needs `mutate`.

use std::sync::{Arc, Mutex};

use crate::schema::Answers;
use ljos_mcp_core::{
    CallCtx, Cap, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, denied, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use studio_core::{Address, CommentView};

use crate::{Studio, error, lock};

const NAME: &str = "review";

#[derive(Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Action {
    Add,
    Reply,
    Resolve,
    Reopen,
    List,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReviewArgs {
    action: Action,
    /// The document (`add`; `list` filters by it).
    doc: Option<String>,
    /// The object (`add`; `list` filters by it, with `doc`).
    node: Option<Address>,
    /// The comment's id (`reply`, `resolve`, `reopen`).
    comment: Option<String>,
    /// What the comment or reply says.
    text: Option<String>,
    /// Who says it.
    author: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct AnchorOut {
    /// True of the text the comment was last placed in.
    path: String,
    line: usize,
    #[serde(rename = "type")]
    ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct NoteOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    text: String,
    /// Milliseconds since the Unix epoch.
    at: u64,
}

#[derive(Serialize, JsonSchema)]
struct CommentOut {
    id: String,
    doc: String,
    /// The object's handle; absent once it is orphaned.
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<String>,
    /// Where the object is, or last was.
    anchor: AnchorOut,
    /// The object was taken out; the comment is kept.
    orphaned: bool,
    resolved: bool,
    /// The comment, then its replies.
    thread: Vec<NoteOut>,
}

impl From<CommentView> for CommentOut {
    fn from(v: CommentView) -> Self {
        let c = v.comment;
        Self {
            id: c.id,
            doc: c.doc,
            node: v.node,
            anchor: AnchorOut {
                path: c.anchor.path,
                line: c.anchor.line,
                ty: c.anchor.ty,
                id: c.anchor.id,
            },
            orphaned: c.orphaned,
            resolved: c.resolved,
            thread: c
                .thread
                .into_iter()
                .map(|n| NoteOut {
                    author: n.author,
                    text: n.text,
                    at: n.at,
                })
                .collect(),
        }
    }
}

#[derive(Serialize, JsonSchema)]
struct ReviewOut {
    /// The comment added or changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<CommentOut>,
    /// What `list` found, in the order the comments were made.
    #[serde(skip_serializing_if = "Option::is_none")]
    comments: Option<Vec<CommentOut>>,
}

fn needs<T>(what: Option<T>, name: &str, action: &str) -> Result<T, SurfaceError> {
    what.ok_or_else(|| SurfaceError::invalid_argument(format!("`{action}` needs `{name}`")))
}

fn review(studio: &Mutex<Studio>, ctx: &CallCtx, a: ReviewArgs) -> Result<ReviewOut, SurfaceError> {
    if a.action != Action::List && !ctx.caps.contains(Cap::Mutate) {
        return Err(denied(NAME, Cap::Mutate, ctx));
    }
    let mut studio = lock(studio);
    let at = u64::try_from(studio.timer.wall().as_millis()).unwrap_or(u64::MAX);
    let session = &mut studio.session;
    let author = a.author.as_deref();
    let changed = match a.action {
        Action::List => {
            let node = match (&a.doc, &a.node) {
                (Some(doc), Some(node)) => Some(session.resolve(doc, node).map_err(error)?.0),
                (None, Some(_)) => {
                    return Err(SurfaceError::invalid_argument(
                        "an object is named within a `doc`",
                    ));
                }
                _ => None,
            };
            let comments = session
                .comments(a.doc.as_deref())
                .map_err(error)?
                .into_iter()
                .filter(|c| node.is_none() || c.node == node)
                .map(CommentOut::from)
                .collect();
            return Ok(ReviewOut {
                comment: None,
                comments: Some(comments),
            });
        }
        Action::Add => {
            let doc = needs(a.doc, "doc", "add")?;
            let node = needs(a.node, "node", "add")?;
            let text = needs(a.text, "text", "add")?;
            session.comment_add(&doc, &node, &text, author, at)
        }
        Action::Reply => {
            let id = needs(a.comment, "comment", "reply")?;
            let text = needs(a.text, "text", "reply")?;
            session.comment_reply(&id, &text, author, at)
        }
        Action::Resolve => session.comment_resolve(&needs(a.comment, "comment", "resolve")?, true),
        Action::Reopen => session.comment_resolve(&needs(a.comment, "comment", "reopen")?, false),
    };
    Ok(ReviewOut {
        comment: Some(changed.map_err(error)?.into()),
        comments: None,
    })
}

pub(crate) fn add(registry: ToolRegistry, studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let studio = studio.clone();
    registry.with(
        ToolDef::new::<ReviewArgs>(
            NAME,
            "Design review comments on objects, kept in Studio's sidecar (`.studio/`), never in \
             the QML: add, reply, resolve, reopen, or list by doc and node. A comment follows its \
             object through edits; one whose object is removed stays, orphaned, at its last place.",
        )
        .answers::<ReviewOut>()
        .requires(Cap::Inspect)
        .annotated(ToolAnnotations {
            read_only: false,
            destructive: false,
            idempotent: false,
            open_world: false,
        }),
        typed(move |ctx, a: ReviewArgs| review(&studio, ctx, a)),
    )
}
