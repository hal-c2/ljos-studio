//! The preview tools: `preview_open`, `preview_state`,
//! `preview_screenshot`, `preview_events` (MCP.md), after the edit tools in
//! wire order.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::schema::Answers;
use ljos_mcp_core::{
    Cap, ErrorCode, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, ToolReply, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use studio_core::{
    Address,
    preview::{Alias, Click, PreviewError, PreviewEvent, Reload, Size, Target, ms},
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
    /// The object to read; it needs an `id`. In the previewed document
    /// unless `doc` names another. Left out, the call only clicks: a
    /// prototype plays without an `id` on the object whose state it reads.
    node: Option<Address>,
    doc: Option<String>,
    /// Put `node` in this QML state of its own for the preview, through
    /// reloads, until `""`; the file does not change.
    show: Option<String>,
    /// Values to assign to a block's state (mocked previews only).
    set: Option<Map<String, Value>>,
    /// An object (with an `id`) to click first, after `set`, as a person
    /// does: the app's own handlers answer, so a flow plays.
    click: Option<Address>,
    /// A point to click instead, exactly there: whatever is on top gets it.
    click_at: Option<ClickAt>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClickAt {
    /// The preview's logical pixels.
    x: f64,
    y: f64,
}

#[derive(Serialize, JsonSchema)]
struct StateOut {
    /// `Module.Name`, when the object is a block.
    #[serde(skip_serializing_if = "Option::is_none")]
    block: Option<String>,
    /// The object read; absent when only a click was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    /// A block's published state as the preview has it now.
    state: Map<String, Value>,
    mocked: bool,
    /// The object's QML `state` now: the variant it shows, `""` for its
    /// base state. Absent when no object was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
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
    /// Instead of events, wait until the preview's app rests saying something
    /// new: at most this long (up to 3 600 000), past `rests`.
    wait_ms: Option<u64>,
    /// The `rests` of an earlier answer; left out, the answer comes at once.
    rests: Option<u64>,
}

#[derive(Serialize, JsonSchema)]
struct EventsOut {
    events: Vec<PreviewEvent>,
    /// Pass as `since` for what comes after.
    next: usize,
    /// A preview is running now.
    open: bool,
    /// With `wait_ms`: how many times the app has rested saying something new.
    #[serde(skip_serializing_if = "Option::is_none")]
    rests: Option<u64>,
    /// With `wait_ms`: when the app wakes by itself next; absent for not until
    /// something happens, so its picture will not change by itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    wake_in_ms: Option<u64>,
}

pub(crate) fn preview_error(e: PreviewError) -> SurfaceError {
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
        PreviewError::NotSupported {
            ljos_web_request, ..
        } => SurfaceError::new(ErrorCode::NotActionable, message)
            .with_details(json!({ "kind": "not_supported", "ljos_web_request": ljos_web_request })),
        PreviewError::Transport(_) => kind(ErrorCode::Internal, "transport"),
    }
}

/// Changes the open preview, keeping the record true after each step: the
/// host has done what came before a failure, and a retry does only the rest.
fn change(
    open: &mut Open,
    size: Size,
    target: Option<Target>,
    state: State,
    aliases: &[Alias],
    reloads: &mut Vec<Reload>,
) -> Result<(), PreviewError> {
    if open.size != size {
        open.host.resize(size)?;
        open.size = size;
    }
    open.target = target;
    let old = open.mocked.clone();
    for alias in old.iter().filter(|o| !aliases.contains(o)) {
        reloads.push(open.host.alias(&alias.module, None)?);
        open.mocked.retain(|m| m != alias);
        open.state = if open.mocked.is_empty() {
            State::Live
        } else {
            State::Mocked
        };
    }
    for alias in aliases.iter().filter(|n| !old.contains(n)) {
        reloads.push(open.host.alias(&alias.module, Some(&alias.dir))?);
        open.mocked.push(alias.clone());
        open.state = State::Mocked;
    }
    open.state = state;
    Ok(())
}

fn open(studio: &Mutex<Studio>, a: OpenArgs) -> Result<OpenOut, SurfaceError> {
    // Refused before anything starts or resizes: past the bound is a start
    // failure or an outsized surface in the child, not a clear answer.
    if let Some(size) = a.size.filter(|s| !s.fits()) {
        return Err(SurfaceError::invalid_argument(format!(
            "a size is 1 to {} pixels on each side, not {}x{}",
            Size::MAX_SIDE,
            size.width,
            size.height
        )));
    }
    let studio = lock(studio);
    studio.session.check_document(&a.doc).map_err(error)?;
    if !studio.session.store().exists(&a.doc) {
        return Err(SurfaceError::not_found(format!("no document `{}`", a.doc)));
    }
    let dir = studio.dir();
    let Some(hosts) = studio.hosts.clone().filter(|h| h.runs(dir.as_deref())) else {
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
    let timer = previews.timer.clone();
    let started_at = timer.now();
    let elapsed = || ms(timer.now().saturating_sub(started_at));
    // A process that ended while nothing read it would otherwise be reused
    // as it is when no change needs a host call: ask, and start another.
    if let Some(open) = previews.open.as_mut().filter(|o| o.doc == a.doc)
        && let Err(e) = open.host.alive()
    {
        let e = previews.failed(e);
        if previews.open.is_some() {
            return Err(preview_error(e));
        }
    }
    let reuse = previews.open.as_ref().is_some_and(|o| o.doc == a.doc);
    if reuse {
        let open = previews.open.as_mut().expect("checked");
        let mut reloads = Vec::new();
        let changed = change(open, size, target, state, &aliases, &mut reloads);
        for reload in reloads {
            previews.record(reload, started_at);
        }
        changed.map_err(|e| preview_error(previews.failed(e)))?;
    } else {
        // One preview at a time: the last one ends before the next starts.
        if let Some(mut last) = previews.open.take() {
            last.host.close();
        }
        let mut host = hosts.make(dir.as_deref()).expect("checked: it runs here");
        host.open(&preview::spec(
            studio.session.store(),
            &a.doc,
            size,
            &aliases,
        ))
        .map_err(preview_error)?;
        previews.log_mark = 0;
        previews.events.push(PreviewEvent::Opened {
            entry: a.doc.clone(),
            size,
            mocked: aliases.iter().map(|m| m.module.clone()).collect(),
            ms: elapsed(),
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
            clicks: 0,
            shown: None,
        });
        previews.write_hidden();
    }
    Ok(OpenOut {
        doc: a.doc,
        target,
        size,
        state,
        mocked: aliases.into_iter().map(|m| m.module).collect(),
        started: !reuse,
        ms: elapsed(),
    })
}

fn state(studio: &Mutex<Studio>, a: StateArgs) -> Result<StateOut, SurfaceError> {
    let mut studio = lock(studio);
    let previews = studio.previews.clone();
    let mut previews = preview::caught_up(&previews);
    let open = previews
        .open
        .as_mut()
        .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
    let doc = a.doc.unwrap_or_else(|| open.doc.clone());
    let no_id =
        || SurfaceError::invalid_argument("the preview finds objects by `id`; this one has none");
    let click = match (a.click, a.click_at) {
        (Some(_), Some(_)) => {
            return Err(SurfaceError::invalid_argument(
                "give `click` or `click_at`, not both",
            ));
        }
        (Some(address), None) => {
            let (_, clicked) = studio.session.resolve(&doc, &address).map_err(error)?;
            Some(Click::Id(clicked.id.ok_or_else(no_id)?))
        }
        (None, Some(at)) => Some(Click::Point { x: at.x, y: at.y }),
        (None, None) => None,
    };
    let Some(address) = a.node else {
        if a.set.is_some() {
            return Err(SurfaceError::invalid_argument(
                "`set` needs the `node` whose state it sets",
            ));
        }
        let click = click.ok_or_else(|| {
            SurfaceError::invalid_argument("give the `node` to read, a click, or both")
        })?;
        if let Err(e) = open.host.click(&click) {
            return Err(preview_error(previews.failed(e)));
        }
        open.clicks += 1;
        return Ok(StateOut {
            block: None,
            id: None,
            state: Map::new(),
            mocked: false,
            variant: None,
        });
    };
    let (_, node) = studio.session.resolve(&doc, &address).map_err(error)?;
    let id = node.id.clone().ok_or_else(no_id)?;
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
        .find(|b| b.name == node.ty && imported(&b.module));
    let not_a_block = || {
        SurfaceError::invalid_argument(format!("`{id}` is a `{}`, not a block", node.ty))
            .with_details(json!({ "kind": "not_a_block" }))
    };
    let block = contract.map(|c| format!("{}.{}", c.module, c.name));
    let names: Vec<String> = contract
        .map(|c| c.state.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default();
    let mocked = contract.is_some_and(|c| open.mocked.iter().any(|m| m.module == c.module));
    if let Some(set) = a.set {
        let block = block.as_deref().ok_or_else(not_a_block)?;
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
            if let Err(e) = open.host.write(&id, &name, value) {
                return Err(preview_error(previews.failed(e)));
            }
        }
    }
    if let Some(click) = click {
        if let Err(e) = open.host.click(&click) {
            return Err(preview_error(previews.failed(e)));
        }
        open.clicks += 1;
    }
    if let Some(show) = a.show {
        let known = show.is_empty()
            || studio
                .session
                .variants(&doc, &address)
                .map_err(error)?
                .states
                .iter()
                .any(|s| s.name == show);
        if !known {
            return Err(
                SurfaceError::not_found(format!("`{id}` has no state `{show}`"))
                    .with_details(json!({ "kind": "no_state" })),
            );
        }
        if let Err(e) = open.host.write(&id, "state", Value::String(show.clone())) {
            return Err(preview_error(previews.failed(e)));
        }
        open.shown = (!show.is_empty()).then(|| (id.clone(), show));
        // It changes the picture without a reload, as a click does.
        open.clicks += 1;
    }
    let mut asked = names.clone();
    // A block may publish a `state` of its own; the variant is QML's.
    let qml_state = "state".to_owned();
    if !asked.contains(&qml_state) {
        asked.push(qml_state.clone());
    }
    let mut state = match open.host.read(&id, &asked) {
        Ok(state) => state,
        Err(e) => return Err(preview_error(previews.failed(e))),
    };
    let variant = match state.get(&qml_state) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    state.retain(|name, _| names.contains(name));
    Ok(StateOut {
        block,
        id: Some(id),
        state,
        mocked,
        variant: Some(variant),
    })
}

/// How a picture treats writes a burst has not shown yet.
#[derive(Clone, Copy)]
pub(crate) enum Picturing {
    /// An agent's look: it sees every write, waiting for the budget if a
    /// burst spent it.
    Latest,
    /// A window's: it must not hold up the person typing, and fetches again
    /// when the catch-up gives a new generation.
    AsShown,
}

/// The picture as an MCP image, with its size as the structured answer.
pub(crate) fn screenshot(
    studio: &Mutex<Studio>,
    args: Value,
    picturing: Picturing,
) -> Result<ToolReply, SurfaceError> {
    let a: ScreenshotArgs = serde_json::from_value(if args.is_null() { json!({}) } else { args })
        .map_err(|e| SurfaceError::invalid_argument(e.to_string()))?;
    let side = a
        .max_size
        .unwrap_or(PICTURE_SIDE)
        .clamp(16, MAX_PICTURE_SIDE);
    let studio = lock(studio);
    let mut previews = match picturing {
        Picturing::Latest => preview::caught_up(&studio.previews),
        Picturing::AsShown => {
            let mut previews = preview::lock(&studio.previews);
            previews.catch_up();
            previews
        }
    };
    let open = previews
        .open
        .as_mut()
        .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
    let internal = |e: serde_json::Error| SurfaceError::new(ErrorCode::Internal, e.to_string());
    if let Format::Outline = a.format {
        let outline = match open.host.outline() {
            Ok(outline) => outline,
            Err(e) => return Err(preview_error(previews.failed(e))),
        };
        let out = ScreenshotOut {
            outline: Some(outline),
            ..ScreenshotOut::default()
        };
        return Ok(ToolReply::from(
            serde_json::to_value(out).map_err(internal)?,
        ));
    }
    let picture = match open.host.screenshot(side) {
        Ok(picture) => picture,
        Err(e) => return Err(preview_error(previews.failed(e))),
    };
    let out = ScreenshotOut {
        width: Some(picture.width),
        height: Some(picture.height),
        bytes: Some(picture.png.len()),
        outline: None,
    };
    let structured = serde_json::to_value(out).map_err(internal)?;
    Ok(ToolReply::from(structured).with_image("image/png", picture.png))
}

/// The document object the open preview of `doc` shows at `x`, `y`: as
/// (document, handle), `None` when nothing is there.
pub(crate) fn pick(
    studio: &mut Studio,
    doc: &str,
    x: f64,
    y: f64,
) -> Result<Option<(String, String)>, SurfaceError> {
    let previews = studio.previews.clone();
    // Before anything else: the line the preview answers is looked up in
    // the session's text, and a newly failed reload is caught below.
    let mut previews = preview::caught_up(&previews);
    let not_shown = || {
        SurfaceError::new(
            ErrorCode::NotActionable,
            format!("no preview of `{doc}` is open; `preview_open` starts one"),
        )
        .with_details(json!({ "kind": "no_preview" }))
    };
    if previews.open.as_ref().is_none_or(|o| o.doc != doc) {
        return Err(not_shown());
    }
    // A preview whose last reload failed runs older text than the
    // session's: its lines would name other objects.
    let behind = previews.events.iter().rev().find_map(|e| match e {
        PreviewEvent::Opened { .. } | PreviewEvent::Reloaded { .. } => Some(false),
        PreviewEvent::ReloadFailed { .. } => Some(true),
        _ => None,
    });
    if behind == Some(true) {
        return Err(SurfaceError::new(
            ErrorCode::NotActionable,
            format!("the preview shows an older `{doc}`: its last reload failed"),
        )
        .with_details(json!({ "kind": "preview_behind" })));
    }
    let open = previews.open.as_mut().ok_or_else(not_shown)?;
    let Some(declared) = open.host.pick(x, y).map_err(preview_error)? else {
        return Ok(None);
    };
    drop(previews);
    if !studio.session.store().exists(&declared.file) {
        return Err(SurfaceError::not_found(format!(
            "the object at {x},{y} is declared in `{}`, which is not a document of the project",
            declared.file
        ))
        .with_details(json!({ "kind": "not_a_document" })));
    }
    let (handle, _) = studio
        .session
        .declared_at(
            &declared.file,
            declared.line as usize,
            declared.column as usize,
        )
        .map_err(error)?;
    // A locked object is not to be taken by a click: what lies under it is
    // what the person is after, and the preview names only the topmost.
    if studio.locked.contains(&handle) {
        return Ok(None);
    }
    Ok(Some((declared.file, handle)))
}

/// What the host says of the objects with `ids` and of unnamed ones of
/// `types`; `None` when no preview of `doc` is open or it runs older text
/// than the session's, whose lines would name other objects.
pub(crate) fn placed(
    studio: &mut Studio,
    doc: &str,
    ids: &[String],
    types: &[String],
) -> Result<Option<Vec<studio_core::preview::Placed>>, SurfaceError> {
    let previews = studio.previews.clone();
    let mut previews = preview::caught_up(&previews);
    let behind = previews.events.iter().rev().find_map(|e| match e {
        PreviewEvent::Opened { .. } | PreviewEvent::Reloaded { .. } => Some(false),
        PreviewEvent::ReloadFailed { .. } => Some(true),
        _ => None,
    });
    if behind == Some(true) {
        return Ok(None);
    }
    match previews.open.as_mut().filter(|o| o.doc == doc) {
        Some(open) => open
            .host
            .placed(ids, types)
            .map(Some)
            .map_err(preview_error),
        None => Ok(None),
    }
}

/// Holds the session only to find the preview: the wait is outside the lock,
/// and asks the app nothing else, since a follower asks this many times a second.
fn wait_for_rest(
    studio: &Mutex<Studio>,
    since: usize,
    rests: Option<u64>,
    wait_ms: u64,
) -> Result<EventsOut, SurfaceError> {
    let waiting = {
        let studio = lock(studio);
        let previews = preview::lock(&studio.previews);
        let open = previews
            .open
            .as_ref()
            .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
        open.host.rest_watch().ok_or_else(|| {
            preview_error(PreviewError::NotSupported {
                message: "this preview cannot say when it rests".to_owned(),
                ljos_web_request: 25,
            })
        })?
    };
    let rest = waiting
        .wait(rests, wait_ms.min(3_600_000))
        .map_err(preview_error)?;
    Ok(EventsOut {
        events: Vec::new(),
        next: since,
        open: true,
        rests: Some(rest.rests),
        wake_in_ms: rest.wake_in_ms,
    })
}

/// A wait for a server's thread, begun with the session held only to find it.
#[cfg(feature = "native")]
pub(crate) fn parked(
    studio: &Mutex<Studio>,
    rests: u64,
    wait_ms: u64,
) -> Result<
    impl Future<Output = Result<studio_core::preview::Rest, PreviewError>> + Send + use<>,
    SurfaceError,
> {
    let studio = lock(studio);
    let previews = preview::lock(&studio.previews);
    let open = previews
        .open
        .as_ref()
        .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
    let watch = open.host.rest_watch().ok_or_else(|| {
        preview_error(PreviewError::NotSupported {
            message: "this preview cannot say when it rests".to_owned(),
            ljos_web_request: 25,
        })
    })?;
    Ok(watch.parked(rests, wait_ms.min(3_600_000)))
}

fn events(studio: &Mutex<Studio>, a: EventsArgs) -> Result<EventsOut, SurfaceError> {
    if let Some(wait_ms) = a.wait_ms {
        return wait_for_rest(studio, a.since.unwrap_or(0), a.rests, wait_ms);
    }
    let studio = lock(studio);
    // What a burst of writes left out is reloaded first, so its outcome
    // is among the events.
    let mut previews = preview::caught_up(&studio.previews);
    previews.pull_logs();
    let since = a.since.unwrap_or(0).min(previews.events.len());
    let end = (since + EVENTS_PAGE).min(previews.events.len());
    Ok(EventsOut {
        events: previews.events[since..end].to_vec(),
        next: end,
        open: previews.open.is_some(),
        rests: None,
        wake_in_ms: None,
    })
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
            .answers::<OpenOut>()
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
                "Read an object in the preview: its QML state (`variant`) and a block's published \
                 state, which `set` assigns when the block is mocked. `click` or `click_at` clicks \
                 first, as a person does, so a flow plays. Objects are found by `id`.",
            )
            .answers::<StateOut>()
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
            .answers::<ScreenshotOut>()
            .requires(Cap::Inspect)
            .annotated(ToolAnnotations::READ_ONLY),
            typed({
                let studio = s();
                move |_, a: Value| screenshot(&studio, a, Picturing::Latest).map(|r| r.structured)
            }),
        )
        .with(
            ToolDef::new::<EventsArgs>(
                "preview_events",
                "What happened to the preview since a cursor: opened, reloaded (with ms), \
                 reload_failed (the last good one stays), log lines, ended. `wait_ms` instead waits \
                 for it to rest past `rests`, and says `wake_in_ms`.",
            )
            .answers::<EventsOut>()
            .requires(Cap::Inspect)
            .annotated(ToolAnnotations::READ_ONLY),
            typed({
                let studio = s();
                move |_, a: EventsArgs| events(&studio, a)
            }),
        )
}
