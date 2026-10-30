//! The preview tools: `preview_open`, `preview_state`,
//! `preview_screenshot`, `preview_events` (MCP.md), after the edit tools in
//! wire order.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};

use ljos_mcp_core::{
    Cap, ErrorCode, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, ToolReply, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use studio_core::{
    Address,
    preview::{Alias, PreviewError, PreviewEvent, Size, Target, ms},
};

use crate::{
    Studio, blocks, error, lock,
    preview::{self, Open, State},
};

/// Pictures are this many pixels on their longest side unless asked for
/// fewer; the inspector draws at most 1280.
const PICTURE_SIDE: u32 = 800;
const MAX_PICTURE_SIDE: u32 = 1280;
/// Events in one answer at most.
const EVENTS_PAGE: usize = 100;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenArgs {
    /// The document to run, usually a shell (`Shell.qml`).
    doc: String,
    /// The screen to lay it out for (default: the open preview's, else
    /// `desktop`).
    target: Option<Target>,
    /// A size in logical pixels instead of a target's.
    size: Option<Size>,
    /// `live` runs the app's own blocks; `mocked` the project's mock
    /// modules (`mocks/`) in their place. Default: the open preview's, else
    /// `live`.
    state: Option<State>,
}

#[derive(Serialize, JsonSchema)]
struct OpenOut {
    doc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<Target>,
    size: Size,
    state: State,
    /// The modules mock modules stand in for.
    mocked: Vec<String>,
    /// A new process was started; false when the open one was changed.
    started: bool,
    ms: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StateArgs {
    /// The block's object; it needs an `id`. In the previewed document
    /// unless `doc` names another.
    node: Address,
    doc: Option<String>,
    /// Values to assign to the block's state (mocked previews only).
    set: Option<Map<String, Value>>,
}

#[derive(Serialize, JsonSchema)]
struct StateOut {
    /// `Module.Name`.
    block: String,
    id: String,
    /// The block's published state as the preview has it now.
    state: Map<String, Value>,
    mocked: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScreenshotArgs {
    /// `png` (default) or `outline`: what the preview shows as text, one
    /// line per item, for reading rather than looking.
    #[serde(default)]
    format: Format,
    /// Longest side of a PNG in pixels (default 800, at most 1280).
    max_size: Option<u32>,
}

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Format {
    #[default]
    Png,
    Outline,
}

#[derive(Default, Serialize, JsonSchema)]
struct ScreenshotOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outline: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EventsArgs {
    /// The `next` of an earlier answer; 0 or left out: from the start.
    since: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
struct EventsOut {
    events: Vec<PreviewEvent>,
    /// Pass as `since` for what comes after.
    next: usize,
    /// A preview is running now.
    open: bool,
}

fn preview_error(e: PreviewError) -> SurfaceError {
    let message = e.to_string();
    let kind = |code: ErrorCode, kind: &str| {
        SurfaceError::new(code, message.clone()).with_details(json!({ "kind": kind }))
    };
    match e {
        PreviewError::NotOpen => kind(ErrorCode::NotActionable, "no_preview"),
        PreviewError::Start(_) => kind(ErrorCode::NotActionable, "start"),
        PreviewError::Ended(_) => kind(ErrorCode::NotActionable, "ended"),
        PreviewError::NoObject(_) => kind(ErrorCode::NotFound, "no_object"),
        PreviewError::Refused { code, .. } => SurfaceError::new(ErrorCode::NotActionable, message)
            .with_details(json!({ "kind": "refused", "code": code })),
        PreviewError::Transport(_) => kind(ErrorCode::Internal, "transport"),
    }
}

fn open(studio: &Mutex<Studio>, a: OpenArgs) -> Result<OpenOut, SurfaceError> {
    let studio = lock(studio);
    studio.session.check_document(&a.doc).map_err(error)?;
    if !studio.session.store().exists(&a.doc) {
        return Err(SurfaceError::not_found(format!("no document `{}`", a.doc)));
    }
    let (Some(hosts), Some(dir)) = (studio.hosts.clone(), studio.dir()) else {
        return Err(SurfaceError::new(
            ErrorCode::NotActionable,
            "this server has no preview host",
        ));
    };
    let mocks = preview::mocks(studio.session.store());
    let mut previews = preview::lock(&studio.previews);
    let previous = previews.open.as_ref();
    let target = a.target.or_else(|| {
        if a.size.is_some() {
            None
        } else {
            previous.and_then(|o| o.target)
        }
    });
    let size = a
        .size
        .or_else(|| a.target.map(Target::size))
        .or_else(|| previous.map(|o| o.size))
        .unwrap_or_else(|| Target::Desktop.size());
    let target = if a.size.is_none() && target.is_none() && previous.is_none() {
        Some(Target::Desktop)
    } else {
        target
    };
    let state = a
        .state
        .or_else(|| previous.map(|o| o.state))
        .unwrap_or(State::Live);
    let aliases: Vec<Alias> = match state {
        State::Live => Vec::new(),
        State::Mocked if mocks.is_empty() => {
            return Err(SurfaceError::new(
                ErrorCode::NotActionable,
                "the project has no mock modules: a `mocks/` directory with a qmldir per module",
            )
            .with_details(json!({ "kind": "no_mocks" })));
        }
        State::Mocked => mocks,
    };
    let started_at = Instant::now();
    let reuse = previous.is_some_and(|o| o.doc == a.doc);
    if reuse {
        let open = previews.open.as_mut().expect("checked");
        if open.size != size {
            open.host.resize(size).map_err(preview_error)?;
            open.size = size;
        }
        let old = std::mem::take(&mut open.mocked);
        let mut reloads = Vec::new();
        for alias in old.iter().filter(|o| !aliases.contains(o)) {
            reloads.push(
                open.host
                    .alias(&alias.module, None)
                    .map_err(preview_error)?,
            );
        }
        for alias in aliases.iter().filter(|n| !old.contains(n)) {
            reloads.push(
                open.host
                    .alias(&alias.module, Some(&alias.dir))
                    .map_err(preview_error)?,
            );
        }
        open.mocked = aliases.clone();
        open.state = state;
        open.target = target;
        for reload in reloads {
            previews.record(reload, started_at);
        }
    } else {
        // One preview at a time: the last one ends before the next starts.
        if let Some(mut last) = previews.open.take() {
            last.host.close();
        }
        let mut host = hosts(&dir);
        host.open(&preview::spec(&a.doc, size, &aliases))
            .map_err(preview_error)?;
        previews.log_mark = 0;
        previews.events.push(PreviewEvent::Opened {
            entry: a.doc.clone(),
            size,
            mocked: aliases.iter().map(|m| m.module.clone()).collect(),
            ms: ms(started_at.elapsed()),
        });
        previews.open = Some(Open {
            host,
            doc: a.doc.clone(),
            target,
            size,
            state,
            mocked: aliases.clone(),
            reloads: VecDeque::new(),
            behind: false,
        });
    }
    Ok(OpenOut {
        doc: a.doc,
        target,
        size,
        state,
        mocked: aliases.into_iter().map(|m| m.module).collect(),
        started: !reuse,
        ms: ms(started_at.elapsed()),
    })
}

fn state(studio: &Mutex<Studio>, a: StateArgs) -> Result<StateOut, SurfaceError> {
    let mut studio = lock(studio);
    let previews = studio.previews.clone();
    let mut previews = preview::lock(&previews);
    previews.catch_up();
    let open = previews
        .open
        .as_mut()
        .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
    let doc = a.doc.unwrap_or_else(|| open.doc.clone());
    let (_, node) = studio.session.resolve(&doc, &a.node).map_err(error)?;
    let id = node.id.clone().ok_or_else(|| {
        SurfaceError::invalid_argument("the preview finds objects by `id`; this one has none")
    })?;
    let registry = blocks::registry(studio.session.store());
    let text = studio.session.text(&doc).map_err(error)?.0.to_owned();
    let imported = |module: &str| {
        text.lines().any(|l| {
            let mut words = l.split_whitespace();
            words.next() == Some("import") && words.next() == Some(module)
        })
    };
    let contract = registry
        .blocks()
        .find(|b| b.name == node.ty && imported(&b.module))
        .ok_or_else(|| {
            SurfaceError::invalid_argument(format!("`{id}` is a `{}`, not a block", node.ty))
                .with_details(json!({ "kind": "not_a_block" }))
        })?;
    let block = format!("{}.{}", contract.module, contract.name);
    let names: Vec<String> = contract.state.iter().map(|s| s.name.clone()).collect();
    let mocked = open.mocked.iter().any(|m| m.module == contract.module);
    if let Some(set) = a.set {
        if !mocked {
            return Err(SurfaceError::new(
                ErrorCode::NotActionable,
                format!(
                    "`{block}` runs live: its state is the app's own; preview it mocked to set it"
                ),
            )
            .with_details(json!({ "kind": "live" })));
        }
        if let Some(unknown) = set.keys().find(|k| !names.contains(k)) {
            return Err(SurfaceError::invalid_argument(format!(
                "`{unknown}` is not state of `{block}` (its state: {})",
                names.join(", ")
            )));
        }
        for (name, value) in set {
            open.host.write(&id, &name, value).map_err(preview_error)?;
        }
    }
    let state = open.host.read(&id, &names).map_err(preview_error)?;
    Ok(StateOut {
        block,
        id,
        state,
        mocked,
    })
}

/// The picture as an MCP image, with its size as the structured answer.
pub(crate) fn screenshot(studio: &Mutex<Studio>, args: Value) -> Result<ToolReply, SurfaceError> {
    let a: ScreenshotArgs = serde_json::from_value(if args.is_null() { json!({}) } else { args })
        .map_err(|e| SurfaceError::invalid_argument(e.to_string()))?;
    let side = a
        .max_size
        .unwrap_or(PICTURE_SIDE)
        .clamp(16, MAX_PICTURE_SIDE);
    let studio = lock(studio);
    let mut previews = preview::lock(&studio.previews);
    previews.catch_up();
    let open = previews
        .open
        .as_mut()
        .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
    let internal = |e: serde_json::Error| SurfaceError::new(ErrorCode::Internal, e.to_string());
    if let Format::Outline = a.format {
        let outline = open.host.outline().map_err(preview_error)?;
        let out = ScreenshotOut {
            outline: Some(outline),
            ..ScreenshotOut::default()
        };
        return Ok(ToolReply::from(
            serde_json::to_value(out).map_err(internal)?,
        ));
    }
    let picture = open.host.screenshot(side).map_err(preview_error)?;
    let out = ScreenshotOut {
        width: Some(picture.width),
        height: Some(picture.height),
        bytes: Some(picture.png.len()),
        outline: None,
    };
    let structured = serde_json::to_value(out).map_err(internal)?;
    Ok(ToolReply::from(structured).with_image("image/png", picture.png))
}

fn events(studio: &Mutex<Studio>, a: EventsArgs) -> EventsOut {
    let studio = lock(studio);
    let mut previews = preview::lock(&studio.previews);
    // What a burst of writes left out is reloaded first, so its outcome
    // is among the events.
    previews.catch_up();
    previews.pull_logs();
    let since = a.since.unwrap_or(0).min(previews.events.len());
    let end = (since + EVENTS_PAGE).min(previews.events.len());
    EventsOut {
        events: previews.events[since..end].to_vec(),
        next: end,
        open: previews.open.is_some(),
    }
}

pub(crate) fn add(registry: ToolRegistry, studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let s = || studio.clone();
    registry
        .with(
            ToolDef::new::<OpenArgs>(
                "preview_open",
                "Run a document (a shell) in a preview for a target (desktop, tablet, phone) with \
                 live blocks or the project's mock modules. On the open preview it changes size or \
                 state in place, keeping the app's state. Edits show in it at once.",
            )
            .output::<OpenOut>()
            .requires(Cap::Run)
            .annotated(ToolAnnotations::WRITE),
            typed({
                let studio = s();
                move |_, a: OpenArgs| open(&studio, a)
            }),
        )
        .with(
            ToolDef::new::<StateArgs>(
                "preview_state",
                "Read a block's published state in the preview, or `set` it when the block is \
                 mocked. The block's object needs an `id`.",
            )
            .output::<StateOut>()
            .requires(Cap::Run)
            .annotated(ToolAnnotations::WRITE),
            typed({
                let studio = s();
                move |_, a: StateArgs| state(&studio, a)
            }),
        )
        .with(
            ToolDef::new::<ScreenshotArgs>(
                "preview_screenshot",
                "What the preview shows: a PNG at most `max_size` pixels on its longest side \
                 (default 800), or with `format: \"outline\"` its items as text, one per line.",
            )
            .output::<ScreenshotOut>()
            .requires(Cap::Inspect)
            .annotated(ToolAnnotations::READ_ONLY),
            typed({
                let studio = s();
                move |_, a: Value| screenshot(&studio, a).map(|r| r.structured)
            }),
        )
        .with(
            ToolDef::new::<EventsArgs>(
                "preview_events",
                "What happened to the preview since a cursor: opened, reloaded (with ms), \
                 reload_failed (the last good one stays), log lines, ended.",
            )
            .output::<EventsOut>()
            .requires(Cap::Inspect)
            .annotated(ToolAnnotations::READ_ONLY),
            typed({
                let studio = s();
                move |_, a: EventsArgs| Ok::<_, SurfaceError>(events(&studio, a))
            }),
        )
}
