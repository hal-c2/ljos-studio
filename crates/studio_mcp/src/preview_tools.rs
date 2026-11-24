//! The preview tools: `preview_open`, `preview_state`,
//! `preview_screenshot`, `preview_events` (MCP.md), after the edit tools in
//! wire order.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::schema::Answers;
use ljos_mcp_core::{
    CallCtx, Cap, ErrorCode, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, ToolReply, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use studio_core::{
    Address,
    preview::{
        Alias, Click, Input, PreviewError, PreviewEvent, Reload, RestWatch, Size, Target, ms,
    },
};

use crate::{
    Studio, blocks, error, lock,
    preview::{self, Fit, Open, State},
    view::{self, PictureOf},
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
    /// The document, usually `Shell.qml`.
    doc: String,
    /// Default: the open preview's, else `desktop`.
    target: Option<Target>,
    /// Logical pixels, instead of a target's.
    size: Option<Size>,
    /// `live` runs the app's blocks; `mocked` the mock modules (`mocks/`) in their place. Default: the open preview's, else `live`.
    state: Option<State>,
    /// `content`: as tall as the page.
    fit: Option<Fit>,
}

#[derive(Serialize, JsonSchema)]
struct OpenOut {
    doc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<Target>,
    size: Size,
    state: State,
    fit: Fit,
    /// The modules mock modules stand in for.
    mocked: Vec<String>,
    /// A new process was started; false when the open one was changed.
    started: bool,
    /// False when a mock switch started the app over on a host that cannot keep its state.
    state_kept: bool,
    ms: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StateArgs {
    /// The object to read, by `id`; in the previewed document unless `doc` names another.
    node: Option<Address>,
    doc: Option<String>,
    /// Show `node` in this QML state through reloads, until `""`.
    show: Option<String>,
    /// Properties of `node` to read into `state`.
    #[serde(default)]
    read: Vec<String>,
    /// Values to assign to a block's state (mocked previews only).
    set: Option<Map<String, Value>>,
    /// An object, by `id`, to click first, after `set`.
    click: Option<Address>,
    /// A point to click instead: what is on top gets it.
    click_at: Option<ClickAt>,
    /// Wheel notches; negative `dy` is down.
    scroll: Option<Wheel>,
    /// A drag, in pixels.
    swipe: Option<Wheel>,
    /// Keys: `Ctrl+A Enter`.
    press: Option<String>,
    #[serde(rename = "type")]
    typed: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Wheel {
    x: f64,
    y: f64,
    dx: Option<f64>,
    dy: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClickAt {
    /// Logical pixels.
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
    /// `png` (default) or `outline`: one text line per item.
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
    /// Instead of events, wait up to this long (3 600 000 at most) for the app to rest past `rests`.
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
    // Against the screen, not the size laid out: fitted to content that is
    // taller, and an unchanged screen is no resize. The screen is the one
    // asked for only once the host took it, so a refused one is asked again.
    if open.screen != size {
        open.host.resize(size)?;
        open.size = size;
        open.screen = size;
        // What was measured belongs to the page at the old size.
        open.measured = None;
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
        .or_else(|| previous.map(|o| o.screen))
        .unwrap_or_else(|| Target::Desktop.size());
    let fit = a
        .fit
        .or_else(|| previous.map(|o| o.fit))
        .unwrap_or_default();
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
    let store = studio.session.store();
    let harness = crate::harness::harness(store, &a.doc);
    // One that has come to need a harness cannot load where it runs, and
    // one that no longer does is broken by what its old harness sets.
    let reuse = previews
        .open
        .as_ref()
        .is_some_and(|o| o.doc == a.doc && harness.is_some() == o.harness.is_some());
    let mut state_kept = true;
    let write_harness = |h: &crate::harness::Harness| {
        if store.read(&h.path).ok().as_deref() == Some(h.text.as_str()) {
            return Ok(());
        }
        store.write(&h.path, &h.text).map_err(|e| {
            SurfaceError::new(
                ErrorCode::Internal,
                format!("the preview's harness `{}`: {e}", h.path),
            )
        })
    };
    if reuse {
        let open = previews.open.as_mut().expect("checked");
        // What the document requires may have changed since: the running
        // harness follows it, as the preview follows the document.
        if let Some(h) = harness
            .as_ref()
            .filter(|h| open.harness.as_ref().is_some_and(|s| s.path == h.path))
        {
            write_harness(h)?;
        }
        let mut reloads = Vec::new();
        let same = open.screen == size && open.fit == fit;
        let was = open.fit;
        let before = open.size;
        let changed = change(open, size, target, state, &aliases, &mut reloads);
        // A new screen, fit or alias is laid out from the screen again; the
        // same ones keep the height fitted, not shrunk and grown back.
        let changed = changed.and_then(|()| {
            open.fit = fit;
            if same && reloads.is_empty() {
                open.keep_fit()
            } else {
                open.refit()
            }
        });
        // A fit the host refused, or one asked with a change it refused, is
        // not kept, or every later open that names no fit would be refused
        // for it; nor is what it had grown the preview to.
        if changed.is_err() {
            open.undo_fit(was, before);
        }
        // A mock switched in starts the app again, with everything showing.
        let restarted = reloads
            .iter()
            .any(|reload| matches!(reload, Reload::Applied { .. }));
        state_kept = !(restarted && open.host.restarts_on_switch());
        for reload in reloads {
            previews.record(reload, started_at);
        }
        changed.map_err(|e| preview_error(previews.failed(e)))?;
        if restarted {
            previews.write_hidden();
        }
    } else {
        // One preview at a time: the last one ends before the next starts.
        if let Some(mut last) = previews.open.take() {
            last.host.close();
            if let Some(standing) = &mut last.harness {
                standing.leave(store);
            }
        }
        let mut standing = match &harness {
            Some(h) => {
                // Held before it is written: another Studio whose preview
                // of it ends meanwhile then leaves it.
                let standing = h.standing(dir.as_deref()).ok_or_else(|| {
                    SurfaceError::new(
                        ErrorCode::NotActionable,
                        format!(
                            "the preview's harness `{}` cannot be held: Studio makes a lock file beside it and `.studio/preview.lock`",
                            h.path
                        ),
                    )
                    .with_details(json!({ "kind": "harness_unheld" }))
                })?;
                write_harness(h)?;
                Some(standing)
            }
            None => None,
        };
        let entry = harness.as_ref().map_or(a.doc.as_str(), |h| h.path.as_str());
        let mut host = hosts.make(dir.as_deref()).expect("checked: it runs here");
        if let Err(e) = host.open(&preview::spec(store, entry, size, &aliases)) {
            if let Some(standing) = &mut standing {
                standing.leave(store);
            }
            return Err(preview_error(e));
        }
        previews.log_mark = 0;
        previews.events.push(PreviewEvent::Opened {
            entry: a.doc.clone(),
            size,
            mocked: aliases.iter().map(|m| m.module.clone()).collect(),
            ms: elapsed(),
        });
        previews.starts += 1;
        previews.open = Some(Open {
            started: previews.starts,
            host,
            doc: a.doc.clone(),
            target,
            size,
            screen: size,
            fit,
            measured: None,
            state,
            mocked: aliases.clone(),
            reloads: VecDeque::new(),
            behind: false,
            clicks: 0,
            shown: None,
            breakpoint: None,
            harness: standing,
        });
        previews.write_hidden();
        let open = previews.open.as_mut().expect("just opened");
        if let Err(e) = open.refit() {
            open.undo_fit(Fit::Screen, size);
            return Err(preview_error(previews.failed(e)));
        }
    }
    let size = previews.open.as_ref().map_or(size, |o| o.size);
    Ok(OpenOut {
        doc: a.doc,
        target,
        size,
        state,
        fit,
        mocked: aliases.into_iter().map(|m| m.module).collect(),
        started: !reuse,
        state_kept,
        ms: elapsed(),
    })
}

/// Gives the running app a click, then the other input, in that order, as
/// a person's hands would: the click gives a field the keyboard first.
fn act(open: &mut Open, click: Option<&Click>, inputs: &[Input]) -> Result<(), PreviewError> {
    if let Some(click) = click {
        open.host.click(click)?;
        open.clicks += 1;
    }
    for input in inputs {
        open.host.input(input)?;
        // It changes the picture without a reload, as a click does.
        open.clicks += 1;
    }
    Ok(())
}

/// How long an agent's read after a scroll or a swipe waits at most for
/// the page to stop: a fling that outlasts it is read where it has got to.
const SETTLE_MS: u64 = 3000;

/// What a `preview_state` call reads once its input is in.
struct ToRead {
    doc: String,
    address: Address,
    /// The object the address named when the input went in: one put in its
    /// place while the page settled is another, and is not read for it.
    handle: String,
    id: String,
    block: Option<String>,
    /// The block's module, to say once it settles whether it runs mocked:
    /// aliases may have come or gone meanwhile.
    module: Option<String>,
    /// The block's published state, then what `read` asked for.
    names: Vec<String>,
    show: Option<String>,
}

/// A `preview_state` call with its input in.
struct Acted {
    /// The preview the input went to, by its `started`.
    started: u64,
    to_read: Option<ToRead>,
    /// What a scroll or a swipe set moving, for a caller that waits for it.
    watch: Option<Box<dyn RestWatch>>,
}

/// In three steps, so a glide holds up no other tool: the input goes in
/// under Studio's lock; an agent's read then waits for the page to stop
/// outside it, as a person looks once it stops; and what is read is read
/// under the lock again. Studio's window (`PERSON`) does not wait: a hand
/// turning the wheel goes on turning it while the page glides.
fn state(studio: &Mutex<Studio>, ctx: &CallCtx, a: StateArgs) -> Result<StateOut, SurfaceError> {
    let waits = ctx.client.as_deref() != Some(studio_core::PERSON);
    let Acted {
        started,
        to_read,
        watch,
    } = state_input(studio, a, waits)?;
    if let Some(watch) = watch
        && let Err(e) = watch.settle(SETTLE_MS)
    {
        let previews = lock(studio).previews.clone();
        let mut previews = preview::lock(&previews);
        // One opened meanwhile did not fail: the wait was on the one before.
        if previews.open.as_ref().map(|open| open.started) != Some(started) {
            return Err(replaced());
        }
        // The wait asks the inspector alone; whether the program behind it
        // ended, only the host can tell.
        let e = match (e, previews.open.as_mut()) {
            (PreviewError::Transport(message), Some(open)) => open
                .host
                .alive()
                .err()
                .unwrap_or(PreviewError::Transport(message)),
            (e, _) => e,
        };
        return Err(preview_error(previews.failed(e)));
    }
    match to_read {
        Some(to_read) => state_read(studio, started, to_read),
        None => Ok(StateOut {
            block: None,
            id: None,
            state: Map::new(),
            mocked: false,
            variant: None,
        }),
    }
}

/// The preview an input went to was replaced while its page settled, by a
/// call that came in meanwhile: what the new one shows is another app's.
fn replaced() -> SurfaceError {
    SurfaceError::new(
        ErrorCode::StaleRef,
        "another preview was opened while the page settled; nothing was read",
    )
    .with_details(json!({ "kind": "preview_replaced" }))
}

/// Puts the call's writes and input in, and says what to read after and
/// what to wait on first: what a scroll or a swipe set moving, if `waits`.
fn state_input(studio: &Mutex<Studio>, a: StateArgs, waits: bool) -> Result<Acted, SurfaceError> {
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
    let inputs: Vec<Input> = [
        a.scroll.map(|w| Input::Scroll {
            x: w.x,
            y: w.y,
            dx: w.dx.unwrap_or(0.0),
            dy: w.dy.unwrap_or(0.0),
        }),
        a.swipe.map(|w| Input::Swipe {
            x: w.x,
            y: w.y,
            dx: w.dx.unwrap_or(0.0),
            dy: w.dy.unwrap_or(0.0),
        }),
        a.press.map(Input::Press),
        a.typed.map(Input::Type),
    ]
    .into_iter()
    .flatten()
    .collect();
    let Some(address) = a.node else {
        if a.set.is_some() {
            return Err(SurfaceError::invalid_argument(
                "`set` needs the `node` whose state it sets",
            ));
        }
        if click.is_none() && inputs.is_empty() {
            return Err(SurfaceError::invalid_argument(
                "give the `node` to read, input (a click, scroll, swipe, press or type), or both",
            ));
        }
        if let Err(e) = act(open, click.as_ref(), &inputs) {
            return Err(preview_error(previews.failed(e)));
        }
        return Ok(Acted {
            started: open.started,
            to_read: None,
            watch: settling(open, &inputs, waits),
        });
    };
    let (handle, node) = studio.session.resolve(&doc, &address).map_err(error)?;
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
    if let Err(e) = act(open, click.as_ref(), &inputs) {
        return Err(preview_error(previews.failed(e)));
    }
    let mut names = names;
    names.extend(a.read);
    let to_read = ToRead {
        doc,
        address,
        handle,
        id,
        block,
        module: contract.map(|c| c.module.clone()),
        names,
        show: a.show,
    };
    Ok(Acted {
        started: open.started,
        to_read: Some(to_read),
        watch: settling(open, &inputs, waits),
    })
}

/// What to wait on before reading: the preview, if an input set something
/// moving and the caller waits for it to stop.
fn settling(open: &Open, inputs: &[Input], waits: bool) -> Option<Box<dyn RestWatch>> {
    let moves = inputs
        .iter()
        .any(|i| matches!(i, Input::Scroll { .. } | Input::Swipe { .. }));
    if waits && moves {
        open.host.rest_watch()
    } else {
        None
    }
}

/// Shows and reads what the call asked for, once its input is in, in the
/// preview it went to (`started`).
fn state_read(studio: &Mutex<Studio>, started: u64, r: ToRead) -> Result<StateOut, SurfaceError> {
    let ToRead {
        doc,
        address,
        handle,
        id,
        block,
        module,
        names,
        show,
    } = r;
    let mut studio = lock(studio);
    let previews = studio.previews.clone();
    let mut previews = preview::caught_up(&previews);
    let open = previews
        .open
        .as_mut()
        .ok_or_else(|| preview_error(PreviewError::NotOpen))?;
    if open.started != started {
        return Err(replaced());
    }
    // Edits that came in while the page settled may have removed the object
    // and given its id to another.
    let (now, _) = studio.session.resolve(&doc, &address).map_err(error)?;
    if now != handle {
        return Err(SurfaceError::new(
            ErrorCode::StaleRef,
            format!(
                "`{id}` was removed while the page settled and another has its id now; nothing was read"
            ),
        )
        .with_details(json!({ "kind": "node_replaced" })));
    }
    let mocked = module
        .as_deref()
        .is_some_and(|m| open.mocked.iter().any(|a| a.module == m));
    if let Some(show) = show {
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
        // Nothing asked for any more: back to what the width turns on.
        if show.is_empty()
            && let Some((root, state)) = &open.breakpoint
            && let Err(e) = open.host.write(root, "state", Value::String(state.clone()))
        {
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
    shown_screenshot(studio, args, picturing).map(|(reply, _)| reply)
}

/// [`screenshot`], with what the picture was taken of: none for an outline.
pub(crate) fn shown_screenshot(
    studio: &Mutex<Studio>,
    args: Value,
    picturing: Picturing,
) -> Result<(ToolReply, Option<PictureOf>), SurfaceError> {
    let a: ScreenshotArgs = serde_json::from_value(if args.is_null() { json!({}) } else { args })
        .map_err(|e| SurfaceError::invalid_argument(e.to_string()))?;
    let side = a
        .max_size
        .unwrap_or(PICTURE_SIDE)
        .clamp(16, MAX_PICTURE_SIDE);
    let studio = lock(studio);
    // Its own handle: the document's generation is read while it is held.
    let all = studio.previews.clone();
    let mut previews = match picturing {
        Picturing::Latest => preview::caught_up(&all),
        Picturing::AsShown => {
            let mut previews = preview::lock(&all);
            previews.catch_up();
            previews
        }
    };
    // Read before the picture is taken and without reading the file: an
    // edit landing while it is taken is newer than its pixels.
    let shown = view::preview_of(&previews).map(|p| PictureOf {
        text: studio.session.held_generation(&p.doc).unwrap_or(0),
        doc: p.doc,
        target: p.target,
    });
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
        return Ok((
            ToolReply::from(serde_json::to_value(out).map_err(internal)?),
            None,
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
    Ok((
        ToolReply::from(structured).with_image("image/png", picture.png),
        shown,
    ))
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
    let Some(mut declared) = open.host.pick(x, y).map_err(preview_error)? else {
        return Ok(None);
    };
    // The stand-in's one instance of the document is the document's root: it
    // is written there only because nothing else makes one, and a runtime
    // names an instance's root where the instance is written.
    let in_harness = open
        .harness
        .as_ref()
        .is_some_and(|h| h.path == declared.file);
    let instance = in_harness
        && studio
            .session
            .store()
            .read(&declared.file)
            .is_ok_and(|text| crate::harness::instance_line(&text) == Some(declared.line as usize));
    if instance {
        drop(previews);
        let (tree, _) = studio.session.tree(doc).map_err(error)?;
        let Some((handle, _)) = tree.into_iter().next() else {
            return Ok(None);
        };
        if studio.locked.contains(&handle) {
            return Ok(None);
        }
        return Ok(Some((doc.to_owned(), handle)));
    }
    // Around the instance is only the stand-in's own frame: no part of the
    // document, and not one to edit either.
    if in_harness {
        return Ok(None);
    }
    let mut scene = None;
    if studio.session.store().exists(&declared.file) {
        let (_, node) = studio
            .session
            .declared_at(
                &declared.file,
                declared.line as usize,
                declared.column as usize,
            )
            .map_err(error)?;
        // The picture of a 3D view is one object to the preview; the model
        // under the point is found from the scene it reads.
        if node.ty.rsplit('.').next() == Some("View3D") {
            scene = open.host.scene(&declared).map_err(preview_error)?;
        }
    }
    if let Some(body) = scene.as_ref().and_then(|s| s.pick(x, y).ok().flatten()) {
        declared = body.declared.clone();
    }
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

/// The 3D scene around the object declared at `at` of `doc`: `None` when no
/// preview of it is open, it runs older text than the session's, or the
/// object is in no view.
pub(crate) fn scene(
    studio: &mut Studio,
    doc: &str,
    at: &studio_core::preview::Declared,
) -> Result<Option<studio_core::scene3d::Scene>, SurfaceError> {
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
        Some(open) => open.host.scene(at).map_err(preview_error),
        None => Ok(None),
    }
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
                "Run a document in a preview for a target with live blocks or the mock modules. On the open preview it changes size, fit or state in place; `state_kept` says whether the app's state survived.",
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
                "Read an object in the preview: its QML state (`variant`), a block's published state (`set` assigns it when mocked), and the properties in `read`. Input comes first, as a person's.",
            )
            .answers::<StateOut>()
            .requires(Cap::Run)
            .annotated(ToolAnnotations::WRITE),
            typed({
                let studio = s();
                move |ctx: &CallCtx, a: StateArgs| state(&studio, ctx, a)
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
                "The preview's events since a cursor: opened, reloaded (ms), reload_failed (the last good one stays), log lines, ended. `wait_ms` waits instead for it to rest past `rests`, saying `wake_in_ms`.",
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
