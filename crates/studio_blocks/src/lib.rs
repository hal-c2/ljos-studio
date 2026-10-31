//! Studio's blocks: what Studio's shell binds (DESIGN.md, S3 "As built").
//!
//! `Studio.Workspace` publishes the session as plain JS values and takes
//! one action, `dispatch(tool, args)`, which is a Studio tool call through
//! [`StudioServer::call`], the same call an agent makes over MCP. So the
//! shell has no way into the session an agent lacks, and a gesture leaves
//! the same file text and history as the agent's call.
//!
//! `Studio.PreviewPicture` draws the latest picture of the preview a
//! workspace shows.
//!
//! `Studio.ShellStatus` tells the window why the last reload of its shell
//! failed, which a shell that is still running cannot know itself:
//! [`reloaded`] is the host's word for it (DESIGN.md, S4 "As built").
//!
//! [`register`] declares the types, on any Engine (a compile's throwaway
//! one too). [`serve`] hands the real Engine the session; a workspace made
//! without it refuses every gesture.

pub mod shell;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use ljos::item::{self, DisplayListBuilder, ImageHandle, ItemPaint, TextEngine, kurbo};
use ljos::prelude::*;
use ljos::task::{self, Lane};
use ljos_mcp_core::{CallCtx, Cap, CapSet};
use serde_json::{Value as Json, json};
use studio_mcp::StudioServer;
use studio_mcp::view::View;

/// Declares `Studio.Workspace`, `Studio.PreviewPicture` and
/// `Studio.ShellStatus`.
pub fn register(engine: &Engine) {
    engine.register_module(ljos::module!(
        "Studio" 1.0 => [Workspace, PreviewPicture, ShellStatus]
    ));
    item::register_paint_item::<PreviewPicture>(engine, read_picture);
}

/// Gives the workspaces of `engine` the session `server` serves, and has
/// them republish after every write to it, whoever made it: an agent on
/// another thread too. Call it on the Engine's thread, once.
pub fn serve(engine: &Engine, server: Arc<StudioServer>) {
    let context = engine.extension::<Context>();
    let writes = Arc::clone(&context.writes);
    let posted = Arc::new(AtomicBool::new(false));
    let post = Platform::of(engine).poster();
    // One job in the inbox at most, however many writes come before the UI
    // thread runs it: it reads the session as it is by then.
    let watcher: studio_mcp::Watcher = Arc::new(move || {
        writes.fetch_add(1, Ordering::SeqCst);
        if !posted.swap(true, Ordering::SeqCst) {
            let posted = Arc::clone(&posted);
            post.send(move |engine| {
                posted.store(false, Ordering::SeqCst);
                refresh_all(engine);
            });
        }
    });
    lock(&server.studio()).watch(watcher);
    *context.server.borrow_mut() = Some(server);
}

/// What the workspaces of one Engine share.
#[derive(Default)]
struct Context {
    server: RefCell<Option<Arc<StudioServer>>>,
    workspaces: RefCell<Vec<ObjectId>>,
    /// How many times the session told of a change. A view read after the
    /// count was `n` shows every change up to `n`.
    writes: Arc<AtomicU64>,
    statuses: RefCell<Vec<ObjectId>>,
    /// What the last reload said, for a status made after it.
    reload_error: RefCell<String>,
}

/// The session a shell binds: its state as plain JS values, and one
/// action.
#[derive(QmlObject)]
#[qml(
    module = "Studio",
    name = "Workspace",
    block,
    category = "Studio",
    summary = "A Studio session: documents, layers, inspector, palette, tokens, history and preview"
)]
pub struct Workspace {
    /// The project's documents, as paths from its root.
    #[qml(readonly, notify)]
    pub documents: Json,
    /// The document the window shows. The shell sets it; the rest of the
    /// state follows.
    #[qml(notify, change = Self::doc_changed)]
    pub doc: String,
    /// `doc`'s generation, which an edit of its text is based on.
    #[qml(readonly, notify)]
    pub generation: f64,
    #[qml(readonly, notify)]
    pub text: String,
    /// One row per object, in order: `node`, `type`, `id`, `depth`,
    /// `label`, `parent` and `index`, its place as `node_move` counts, and
    /// `comments`, how many open review comments are on it.
    #[qml(readonly, notify)]
    pub layers: Json,
    /// The handle of the selected object of `doc`; empty when none is.
    #[qml(readonly, notify)]
    pub selected: String,
    /// The handles of every selected object of `doc`; `selected` is the first.
    #[qml(readonly, notify)]
    pub chosen: Json,
    /// The selected object's place in `text` (`start`, `end`, in the
    /// characters a script counts).
    #[qml(readonly, notify)]
    pub selection: Json,
    /// The selected object's bindings as written: `name`, `value`.
    #[qml(readonly, notify)]
    pub inspector: Json,
    /// The selected object's states: `name`, `when`?, and `changes`, each
    /// `target`, `name`, `value` as written.
    #[qml(readonly, notify)]
    pub states: Json,
    /// The roles of the data the selected object sees, which a delegate's
    /// bindings read as `model.<role>`.
    #[qml(readonly, notify)]
    pub roles: Json,
    /// The selected object's signals, which a flow may handle: `name`,
    /// `arguments` (each `name`, `type`) and `change` for a property's
    /// change signal.
    #[qml(readonly, notify)]
    pub signals: Json,
    /// The open document's flow: `states`, the root's, the base state
    /// (`""`) first, and `edges`, each `kind` (`transition` or
    /// `interaction`), `from` (`*` for any), `to`, `label`, and an
    /// interaction's `trigger` and `signal` (empty for a `TapHandler`'s).
    #[qml(readonly, notify)]
    pub flow: Json,
    /// The project's blocks: `module`, `name`, `summary`, `category`,
    /// `slots`.
    #[qml(readonly, notify)]
    pub palette: Json,
    /// What undo and redo would take back and do again, latest last.
    #[qml(readonly, notify)]
    pub done: Json,
    #[qml(readonly, notify)]
    pub undone: Json,
    /// `doc`'s review comments: `id`, `node` (empty once orphaned),
    /// `text`, `replies`, `resolved`, `orphaned` and `label`, its object
    /// as the layers panel names it. Each layer counts its open ones in
    /// `comments`.
    #[qml(readonly, notify)]
    pub comments: Json,
    /// The preview of `doc`, or `null`: `target`, `width`, `height`,
    /// `state`, `variant` (the root's state its breakpoint turns on, or
    /// empty), `generation` (what it shows: 1, and one more each reload
    /// it applied) and `picture` (the generation of the picture drawn; 0
    /// for none yet).
    #[qml(readonly, notify)]
    pub preview: Json,
    /// Why the last gesture was refused; empty after one that was answered.
    #[qml(readonly, notify)]
    pub error: String,
    /// Why `doc`'s objects are not listed: its text does not parse.
    #[qml(readonly, notify)]
    pub problem: String,
    /// Whether the session can run previews.
    #[qml(readonly, notify)]
    pub previews: bool,
    /// The theme file `Ljos.Design` reads for `doc`, from the project's
    /// root.
    #[qml(readonly, notify)]
    pub theme: String,
    /// Every role of `Ljos.Design`: `token`, `kind`, `set` (the theme
    /// file sets it), a colour's `light` and `dark`, any other role's
    /// `value` as JSON writes it. Read again after every write, so a
    /// `theme_set` shows at once.
    #[qml(readonly, notify)]
    pub tokens: Json,
    #[qml(skip)]
    queue: Rc<RefCell<Queue>>,
}

/// Each list starts empty, not null, so a shell's bindings read it before
/// the first view arrives as they read it after: a shell that throws while
/// it is made is not shown (`ShellLoader`).
impl Default for Workspace {
    fn default() -> Self {
        let none = || Json::Array(Vec::new());
        Self {
            documents: none(),
            doc: String::new(),
            generation: 0.0,
            text: String::new(),
            layers: none(),
            selected: String::new(),
            chosen: json!([]),
            selection: json!({"start": 0, "end": 0}),
            inspector: none(),
            states: none(),
            roles: none(),
            signals: none(),
            flow: json!({"states": [], "edges": []}),
            palette: none(),
            done: none(),
            undone: none(),
            preview: Json::Null,
            error: String::new(),
            problem: String::new(),
            previews: false,
            theme: String::new(),
            tokens: none(),
            comments: none(),
            queue: Rc::default(),
        }
    }
}

#[qml_impl]
impl Workspace {
    /// A gesture's tool call was answered: the tool's name and its answer.
    #[qml(signal)]
    fn answered(tool: String, answer: Json);

    /// A gesture's tool call was refused: the tool's name and the error
    /// (`code`, `message`, `details`) an agent would be given.
    #[qml(signal)]
    fn refused(tool: String, error: Json);

    /// Calls the Studio tool `tool` with `args`. Calls run in the order
    /// they were made, one at a time, off the UI thread; `answered` or
    /// `refused` tells how each went.
    #[qml(invokable)]
    fn dispatch(cx: &Cx, tool: String, args: Json) -> Result<(), EngineError> {
        enqueue(cx.engine, cx.this, Job::Call { tool, args });
        Ok(())
    }

    #[qml(on_complete)]
    fn completed(cx: &Cx) -> Result<(), EngineError> {
        let context = cx.engine.extension::<Context>();
        context.workspaces.borrow_mut().push(cx.this);
        if context.server.borrow().is_some() {
            enqueue(cx.engine, cx.this, Job::Refresh);
        }
        Ok(())
    }

    fn doc_changed(cx: &Cx, _: PropIndex) {
        enqueue(cx.engine, cx.this, Job::Refresh);
    }
}

/// A workspace's calls waiting their turn, and what it last showed.
#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    /// A job runs off the UI thread.
    busy: bool,
    /// The session's write count the published view was read at.
    published: u64,
    /// The preview generation the last picture was fetched for, drawn or
    /// not, so a picture that cannot be had is not asked for again until
    /// the preview changes.
    fetched: u64,
    image: Option<ImageHandle>,
    /// The generation `image` shows.
    picture: u64,
}

#[derive(Clone)]
enum Job {
    Call {
        tool: String,
        args: Json,
    },
    /// Read the session again.
    Refresh,
    /// Fetch the preview's picture.
    Picture {
        generation: u64,
    },
}

/// What a job did, read off the UI thread.
enum Done {
    Call {
        tool: String,
        args: Json,
        answer: Result<Json, Json>,
        view: View,
        seen: u64,
    },
    Refresh {
        view: View,
        seen: u64,
    },
    Picture {
        generation: u64,
        picture: Result<(u32, u32, Vec<u8>), String>,
    },
}

/// Everything a workspace's calls are allowed: a person at the window
/// may do anything an agent may.
fn caps() -> CapSet {
    CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run])
}

fn enqueue(engine: &Engine, ws: ObjectId, job: Job) {
    let Some(queue) = queue_of(engine, ws) else {
        return;
    };
    {
        let mut q = queue.borrow_mut();
        // A refresh reads the whole session, so one waiting is enough.
        if matches!(job, Job::Refresh) && q.jobs.iter().any(|j| matches!(j, Job::Refresh)) {
            return;
        }
        q.jobs.push_back(job);
    }
    pump(engine, ws);
}

fn queue_of(engine: &Engine, ws: ObjectId) -> Option<Rc<RefCell<Queue>>> {
    engine.with(ws, |w: &Workspace| Rc::clone(&w.queue)).ok()
}

/// Starts the next job when none runs.
fn pump(engine: &Engine, ws: ObjectId) {
    let Some(queue) = queue_of(engine, ws) else {
        return;
    };
    let job = {
        let mut q = queue.borrow_mut();
        if q.busy {
            return;
        }
        let Some(job) = q.jobs.pop_front() else {
            return;
        };
        q.busy = true;
        job
    };
    let context = engine.extension::<Context>();
    let server = context.server.borrow().clone();
    let Some(server) = server else {
        queue.borrow_mut().busy = false;
        if let Job::Call { tool, .. } = job {
            let error = json!({"code": "unavailable", "message": "no Studio session is served"});
            let _ = Workspace::emit_refused(&Cx::new(engine, ws), tool, error);
        }
        return;
    };
    let writes = Arc::clone(&context.writes);
    let doc = engine.get(ws, Workspace::DOC).unwrap_or_default();
    // A person's gestures are the screen's work: the visible lane, ahead
    // of what prefetches.
    let kept = job.clone();
    let spawned = task::spawn_in(
        engine,
        Lane::Visible,
        move || run(&server, &writes, &doc, job),
        move |engine, done| {
            if !engine.is_alive(ws) {
                return;
            }
            if let Some(queue) = queue_of(engine, ws) {
                queue.borrow_mut().busy = false;
            }
            match done {
                Ok(done) => finish(engine, ws, done),
                Err(panic) => {
                    let _ = engine.set(ws, Workspace::ERROR, panic.message().to_owned());
                }
            }
            pump(engine, ws);
        },
    );
    if spawned.is_err() {
        // The pool's queue is full: the job keeps its place, first, and
        // waits for the next gesture or write to ask again.
        let mut q = queue.borrow_mut();
        q.busy = false;
        q.jobs.push_front(kept);
    }
}

/// Runs a job off the UI thread.
fn run(server: &StudioServer, writes: &AtomicU64, doc: &str, job: Job) -> Done {
    let read = || {
        let studio = server.studio();
        let mut studio = lock(&studio);
        // Under the lock no write can come between the count and the view.
        let seen = writes.load(Ordering::SeqCst);
        (studio.view(doc), seen)
    };
    match job {
        Job::Call { tool, args } => {
            let answer = server
                .call(&tool, &CallCtx::new(caps()), args.clone())
                .map(|reply| reply.structured)
                .map_err(|e| refusal(e.to_value()));
            let (view, seen) = read();
            Done::Call {
                tool,
                args,
                answer,
                view,
                seen,
            }
        }
        Job::Refresh => {
            let (view, seen) = read();
            Done::Refresh { view, seen }
        }
        Job::Picture { generation } => {
            let args = json!({"format": "png", "max_size": 1280});
            let picture = server
                .shown_picture(&CallCtx::new(caps()), args)
                .map_err(|e| e.to_string())
                .and_then(|reply| {
                    let image = reply
                        .images
                        .into_iter()
                        .find(|i| i.mime == "image/png")
                        .ok_or_else(|| "the preview answered no PNG".to_owned())?;
                    decode(&image.data)
                });
            Done::Picture {
                generation,
                picture,
            }
        }
    }
}

/// A refusal as the shell is given it: `code`, `message`, `details`. An
/// MCP error wraps them in `error`.
fn refusal(mut error: Json) -> Json {
    match error.get_mut("error") {
        Some(inner) => inner.take(),
        None => error,
    }
}

/// A PNG as straight RGBA, 8 bits a channel.
fn decode(data: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "the picture is too large".to_owned())?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let pixels = (info.width as usize) * (info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("an indexed picture was not expanded".to_owned()),
    };
    debug_assert_eq!(rgba.len(), pixels * 4);
    Ok((info.width, info.height, rgba))
}

/// Publishes what a job did, on the UI thread.
fn finish(engine: &Engine, ws: ObjectId, done: Done) {
    let cx = Cx::new(engine, ws);
    match done {
        Done::Call {
            tool,
            args,
            answer,
            view,
            seen,
        } => {
            if let Ok(answer) = &answer {
                rebase(engine, ws, &tool, &args, answer);
            }
            publish(engine, ws, view, seen);
            // What the canvas reads for itself is no gesture of the person's:
            // it neither clears the refusal of the drop before it nor raises one.
            let gesture = !(tool == "node_arrange" && args["action"] == "geometry");
            match answer {
                Ok(answer) => {
                    if gesture {
                        let _ = cx.set(Workspace::ERROR, String::new());
                    }
                    let _ = Workspace::emit_answered(&cx, tool, answer);
                }
                Err(error) => {
                    if gesture {
                        let message = error["message"].as_str().unwrap_or_default().to_owned();
                        let _ = cx.set(Workspace::ERROR, message);
                    }
                    let _ = Workspace::emit_refused(&cx, tool, error);
                }
            }
        }
        Done::Refresh { view, seen } => publish(engine, ws, view, seen),
        Done::Picture {
            generation,
            picture,
        } => {
            let Some(queue) = queue_of(engine, ws) else {
                return;
            };
            match picture.and_then(|(w, h, rgba)| {
                ImageHandle::from_straight(w, h, rgba).map_err(|e| e.to_string())
            }) {
                Ok(image) => {
                    {
                        let mut q = queue.borrow_mut();
                        q.image = Some(image);
                        q.picture = generation;
                    }
                    let mut preview = engine.get(ws, Workspace::PREVIEW).unwrap_or_default();
                    if let Some(fields) = preview.as_object_mut() {
                        fields.insert("picture".to_owned(), json!(generation));
                        let _ = cx.set(Workspace::PREVIEW, preview);
                    }
                }
                Err(e) => {
                    let _ = cx.set(Workspace::ERROR, format!("no picture of the preview: {e}"));
                }
            }
        }
    }
}

/// A person types on in the code view while their last edit is on its way:
/// the next edit is based on the generation the last one started from,
/// and the person's text already has the last one in it. When that edit
/// was answered, it was applied to exactly that generation (a stale one is
/// refused), so the edits waiting on it are based on its answer instead.
/// An edit based on anything else (an agent wrote in between) is left to
/// be refused, as it must be.
fn rebase(engine: &Engine, ws: ObjectId, tool: &str, args: &Json, answer: &Json) {
    if tool != "document_edit" {
        return;
    }
    let (Some(from), Some(to)) = (args["generation"].as_u64(), answer["generation"].as_u64())
    else {
        return;
    };
    let Some(queue) = queue_of(engine, ws) else {
        return;
    };
    for job in &mut queue.borrow_mut().jobs {
        if let Job::Call { tool, args: next } = job
            && tool == "document_edit"
            && next["doc"] == args["doc"]
            && next["generation"].as_u64() == Some(from)
        {
            next["generation"] = json!(to);
        }
    }
}

fn publish(engine: &Engine, ws: ObjectId, view: View, seen: u64) {
    let Some(queue) = queue_of(engine, ws) else {
        return;
    };
    let cx = Cx::new(engine, ws);
    // The shell may have moved to another document while this was read.
    if engine.get(ws, Workspace::DOC).unwrap_or_default() != view.doc {
        return;
    }
    let (picture, fetched) = {
        let mut q = queue.borrow_mut();
        q.published = q.published.max(seen);
        (q.picture, q.fetched)
    };
    let preview = match &view.preview {
        Some(p) if p.doc == view.doc => json!({
            "target": p.target,
            "width": p.width,
            "height": p.height,
            "generation": p.generation,
            "state": p.state,
            "variant": p.variant,
            "picture": picture,
        }),
        _ => Json::Null,
    };
    let wanted = view
        .preview
        .as_ref()
        .filter(|p| p.doc == view.doc)
        .map(|p| p.generation)
        .filter(|&g| g != picture && g != fetched);
    let _ = cx.set(Workspace::DOCUMENTS, to_json(&view.documents));
    let _ = cx.set(Workspace::GENERATION, view.generation as f64);
    let _ = cx.set(Workspace::TEXT, view.text);
    let _ = cx.set(Workspace::LAYERS, to_json(&view.layers));
    let _ = cx.set(Workspace::SELECTED, view.selected);
    let _ = cx.set(Workspace::CHOSEN, to_json(&view.chosen));
    let _ = cx.set(Workspace::SELECTION, to_json(&view.selection));
    let _ = cx.set(Workspace::INSPECTOR, to_json(&view.inspector));
    let _ = cx.set(Workspace::STATES, to_json(&view.states));
    let _ = cx.set(Workspace::ROLES, to_json(&view.roles));
    let _ = cx.set(Workspace::SIGNALS, to_json(&view.signals));
    let _ = cx.set(Workspace::FLOW, to_json(&view.flow));
    let _ = cx.set(Workspace::PALETTE, to_json(&view.palette));
    let _ = cx.set(Workspace::DONE, to_json(&view.done));
    let _ = cx.set(Workspace::UNDONE, to_json(&view.undone));
    let _ = cx.set(Workspace::COMMENTS, to_json(&view.comments));
    let _ = cx.set(Workspace::PREVIEW, preview);
    let _ = cx.set(Workspace::PROBLEM, view.problem);
    let _ = cx.set(Workspace::PREVIEWS, view.previews);
    let _ = cx.set(Workspace::THEME, view.theme);
    let _ = cx.set(Workspace::TOKENS, to_json(&view.tokens));
    // A picture is fetched once for each generation the preview applies.
    if let Some(generation) = wanted {
        let queued = queue
            .borrow()
            .jobs
            .iter()
            .any(|j| matches!(j, Job::Picture { generation: g } if *g == generation));
        if !queued {
            queue.borrow_mut().fetched = generation;
            enqueue(engine, ws, Job::Picture { generation });
        }
    }
}

/// After a write the session told of: every workspace that has not shown
/// it yet reads the session again.
fn refresh_all(engine: &Engine) {
    let context = engine.extension::<Context>();
    let writes = context.writes.load(Ordering::SeqCst);
    let live: Vec<ObjectId> = {
        let mut all = context.workspaces.borrow_mut();
        all.retain(|&ws| engine.is_alive(ws));
        all.clone()
    };
    for ws in live {
        let behind = queue_of(engine, ws).is_some_and(|q| q.borrow().published < writes);
        if behind {
            enqueue(engine, ws, Job::Refresh);
        }
    }
}

fn lock(
    studio: &std::sync::Mutex<studio_mcp::Studio>,
) -> std::sync::MutexGuard<'_, studio_mcp::Studio> {
    studio
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A view's part as the JSON a script reads.
fn to_json(v: &impl serde::Serialize) -> Json {
    serde_json::to_value(v).unwrap_or_default()
}

/// Why the last reload of the window's shell failed; empty once one was
/// applied or the error went away.
///
/// A reload that does not compile leaves the running shell as it was, so
/// only the host that watches the files knows: it calls [`reloaded`].
#[derive(Default, QmlObject)]
#[qml(module = "Studio", name = "ShellStatus")]
pub struct ShellStatus {
    /// `file:line:column: message`, as Ljós reports it.
    #[qml(readonly, notify)]
    pub reload_error: String,
}

#[qml_impl]
impl ShellStatus {
    #[qml(on_complete)]
    fn completed(cx: &Cx) -> Result<(), EngineError> {
        let context = cx.engine.extension::<Context>();
        context.statuses.borrow_mut().push(cx.this);
        let error = context.reload_error.borrow().clone();
        let _ = cx.set(ShellStatus::RELOAD_ERROR, error);
        Ok(())
    }
}

/// Tells every `Studio.ShellStatus` of `engine` how the last reload of
/// the shell went: `Some` with Ljós's message when it failed, `None` when
/// it was applied or its error went away. Call it on the Engine's thread.
pub fn reloaded(engine: &Engine, error: Option<String>) {
    let context = engine.extension::<Context>();
    let error = error.unwrap_or_default();
    *context.reload_error.borrow_mut() = error.clone();
    let live: Vec<ObjectId> = {
        let mut all = context.statuses.borrow_mut();
        all.retain(|&s| engine.is_alive(s));
        all.clone()
    };
    for status in live {
        // Read-only to QML: only the host says how a reload went.
        let _ = Cx::new(engine, status).set(ShellStatus::RELOAD_ERROR, error.clone());
    }
}

/// Tells every `Studio.ShellStatus` of `engine` the error Ljós holds as
/// standing once a reload or a theme was taken, or that none stands.
/// Ljós knows which: a theme that loads takes away its own error and not
/// a shell's, and a shell that is mended leaves a theme that is still
/// wrong told. Call it on the Engine's thread after each report.
pub fn reload_settled(engine: &Engine) {
    reloaded(engine, engine.last_reload_error().map(|f| f.to_string()));
}

/// The latest picture of the preview a workspace shows, scaled to the
/// item.
#[derive(Default, QmlObject)]
#[qml(module = "Studio", name = "PreviewPicture", extends = Item)]
pub struct PreviewPicture {
    #[qml(base)]
    pub item: Item,
    /// The `Workspace` whose preview it draws.
    #[qml(notify, object = "Workspace")]
    pub workspace: Option<ObjectId>,
}

#[derive(Debug)]
struct Picture(Option<ImageHandle>);

impl ItemPaint for Picture {
    fn paint(&self, size: kurbo::Size, _: &mut TextEngine, out: &mut DisplayListBuilder) {
        if let Some(image) = &self.0 {
            out.image(
                image.clone(),
                kurbo::Rect::from_origin_size((0.0, 0.0), size),
            );
        }
    }
}

fn read_picture(engine: &Engine, obj: ObjectId) -> Rc<dyn ItemPaint> {
    // Both reads are tracked: the item paints again when its workspace or
    // that workspace's preview changes, and not otherwise.
    let ws = item::prop(engine, obj, PreviewPicture::WORKSPACE);
    let image = ws.and_then(|ws| {
        let preview = item::prop(engine, ws, Workspace::PREVIEW);
        let picture = preview["picture"].as_u64().unwrap_or(0);
        let (image, drawn) = queue_of(engine, ws)
            .map(|q| {
                let q = q.borrow();
                (q.image.clone(), q.picture)
            })
            .unwrap_or_default();
        // The image is the one the published picture names.
        (picture == drawn && picture != 0)
            .then_some(image)
            .flatten()
    });
    Rc::new(Picture(image))
}
