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
use std::collections::{HashMap, VecDeque};
use std::hash::Hasher;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ljos::item::{self, DisplayListBuilder, ImageHandle, ItemPaint, TextEngine, kurbo};
use ljos::prelude::*;
use ljos::task::{self, Lane};
use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
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
    /// When a lease shown ends and the workspaces read the session again;
    /// `None` when no wake waits.
    wake_at: std::cell::Cell<Option<Instant>>,
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
    /// The pictures in the project's `assets/`, for the Insert tab.
    #[qml(readonly, notify)]
    pub assets: Json,
    /// The preview's picture may be out of date after a click: the shell
    /// calls `lookAgain` every so often until this is false.
    #[qml(readonly, notify)]
    pub settling: bool,
    /// How many pictures of the preview have been asked for.
    #[qml(readonly, notify)]
    pub looks: i32,
    /// The project's directory has nothing in it yet.
    #[qml(readonly, notify)]
    pub blank: bool,
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
    /// `label`, `parent` and `index`, its place as `node_move` counts,
    /// `comments`, how many open review comments are on it, and `held`,
    /// who else holds it (empty when nobody does).
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
    /// What the selected object can be given: `name`, `type` and, for an
    /// enum, `values` as a file writes them.
    #[qml(readonly, notify)]
    pub offers: Json,
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
    /// The toolkit types the document can place, by module.
    #[qml(readonly, notify)]
    pub toolkit: Json,
    /// The root's states, which stay editable while another object is selected.
    #[qml(readonly, notify)]
    pub root_states: Json,
    /// What other writers hold (`node_select` with `hold`): `doc`, `node`,
    /// `holder`, `ends_in_ms`. A layer another holds, or that is in one,
    /// names the holder in `held`; the window's edit of it is refused.
    #[qml(readonly, notify)]
    pub leases: Json,
    /// What the person's undo and redo would take back and do again,
    /// latest last; an agent's edits are its own to undo.
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
            assets: none(),
            settling: false,
            looks: 0,
            blank: false,
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
            offers: none(),
            flow: json!({"states": [], "edges": []}),
            palette: none(),
            toolkit: none(),
            root_states: none(),
            done: none(),
            undone: none(),
            leases: none(),
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
    /// A gesture's tool call was answered: the tool's name, its answer, and
    /// what it was asked, for a panel that must know the answer is to its call.
    #[qml(signal)]
    fn answered(tool: String, answer: Json, asked: Json);

    /// A gesture's tool call was refused: the tool's name and the error
    /// (`code`, `message`, `details`) an agent would be given, and what it was
    /// asked.
    #[qml(signal)]
    fn refused(tool: String, error: Json, asked: Json);

    /// Calls the Studio tool `tool` with `args`. Calls run in the order
    /// they were made, one at a time, off the UI thread; `answered` or
    /// `refused` tells how each went.
    #[qml(invokable)]
    fn dispatch(cx: &Cx, tool: String, args: Json) -> Result<(), EngineError> {
        enqueue(
            cx.engine,
            cx.this,
            Job::Call {
                tool,
                args,
                follows: false,
            },
        );
        Ok(())
    }

    /// Whether `text` is a literal as QML reads one. The inspector asks before
    /// it writes, and a call could only answer after.
    #[qml(invokable)]
    fn literal(_cx: &Cx, text: String) -> Result<bool, EngineError> {
        Ok(studio_core::is_literal_text(&text))
    }

    /// `dispatch` for a call that follows from what the person just did and
    /// is no gesture of its own: it leaves the refusal on the window's line,
    /// and says its own refusal only when that line is empty.
    #[qml(invokable)]
    fn follow(cx: &Cx, tool: String, args: Json) -> Result<(), EngineError> {
        enqueue(
            cx.engine,
            cx.this,
            Job::Call {
                tool,
                args,
                follows: true,
            },
        );
        Ok(())
    }

    /// Fetches the preview's picture again while `settling`.
    #[qml(invokable, name = "lookAgain")]
    fn look_again(cx: &Cx) -> Result<(), EngineError> {
        let Some(queue) = queue_of(cx.engine, cx.this) else {
            return Ok(());
        };
        let generation = {
            let q = queue.borrow();
            let waiting = q.jobs.iter().any(|j| matches!(j, Job::Picture { .. }));
            (q.settling.is_some() && !waiting).then_some(q.fetched)
        };
        if let Some(generation) = generation {
            enqueue(
                cx.engine,
                cx.this,
                Job::Picture {
                    generation,
                    looks: Vec::new(),
                },
            );
        }
        Ok(())
    }

    /// While on, the picture follows what the preview draws: an app that
    /// animates for ever stays current, and one at rest is asked nothing.
    #[qml(invokable, name = "followPicture")]
    fn follow_picture(cx: &Cx, on: bool) -> Result<(), EngineError> {
        let Some(queue) = queue_of(cx.engine, cx.this) else {
            return Ok(());
        };
        queue.borrow_mut().follow_wanted = on;
        if on {
            follow(cx.engine, cx.this);
        } else {
            let mut q = queue.borrow_mut();
            if let Some(stop) = q.following.take() {
                stop.stop();
                // A thread waiting for a picture to land is told by these going.
            }
        }
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
    /// The preview's size when last published: a new size is a new picture
    /// of the same generation.
    size: (u32, u32),
    /// The document of the preview when last published: each preview counts
    /// its own generations, so another document's number says nothing.
    previewed: String,
    image: Option<ImageHandle>,
    /// The generation `image` shows.
    picture: u64,
    /// What `image`'s pixels hash to, so a look that finds the same picture
    /// draws nothing.
    pixels: u64,
    /// How many different pictures have been drawn.
    shots: u64,
    /// The preview's clicks already looked at.
    clicks: u64,
    settling: Option<Settling>,
    /// Pictures asked for.
    looks: i32,
    /// Play mode wants the picture to follow the preview.
    follow_wanted: bool,
    /// What stops the thread waiting on the preview, while there is one.
    following: Option<Arc<Stop>>,
    /// The lists last set, by the property's name.
    given: HashMap<&'static str, Json>,
}

impl Drop for Queue {
    /// `followPicture(false)` is not called on a Workspace going away, and the
    /// thread would go on waiting on the preview for it.
    fn drop(&mut self) {
        if let Some(stop) = self.following.take() {
            stop.stop();
        }
    }
}

/// Looking again after a click. Nothing tells Studio when the app has come to
/// rest outside play mode, where nothing follows it, so the picture is fetched
/// until it stops changing, a bounded number of times: a click on an app that
/// animates for ever must not cost a screenshot for ever.
struct Settling {
    looks_left: u32,
    same: u32,
}

impl Settling {
    /// A click may take a frame or two to show, so one unchanged picture does
    /// not mean it is over.
    const SAME: u32 = 3;
    const LOOKS: u32 = 30;
}

#[derive(Clone)]
enum Job {
    Call {
        tool: String,
        args: Json,
        follows: bool,
    },
    /// Read the session again.
    Refresh,
    /// Fetch the preview's picture.
    Picture {
        generation: u64,
        /// The follower's looks this picture answers. Theirs alone: one
        /// already being painted when they asked shows the app from before.
        /// A job dropped unrun drops them, and the follower stops waiting.
        looks: Vec<mpsc::Sender<()>>,
    },
}

/// What a job did, read off the UI thread.
enum Done {
    Call {
        tool: String,
        args: Json,
        follows: bool,
        answer: Result<Json, Json>,
        /// With the session's write count it was read at; none for a call
        /// that changes nothing shown.
        view: Option<(View, u64)>,
    },
    Refresh {
        view: View,
        seen: u64,
    },
    Picture {
        generation: u64,
        looks: Vec<mpsc::Sender<()>>,
        /// Width, height, pixels, and what the pixels hash to.
        picture: Result<(u32, u32, Vec<u8>, u64), String>,
    },
}

/// A call from the window: a person, who may do anything an agent may.
/// The name is what the session counts as one writer, so the window's
/// Undo takes the person's edits and an agent's lease holds against them.
fn person() -> CallCtx {
    CallCtx {
        client: Some(studio_mcp::PERSON.to_owned()),
        ..CallCtx::new(CapSet::of([Cap::Inspect, Cap::Mutate, Cap::Run, Cap::Fs]))
    }
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
        if let Job::Call { tool, args, .. } = job {
            let error = json!({"code": "unavailable", "message": "no Studio session is served"});
            let _ = Workspace::emit_refused(&Cx::new(engine, ws), tool, error, args);
        }
        return;
    };
    let writes = Arc::clone(&context.writes);
    let doc = engine.get(ws, Workspace::DOC).unwrap_or_default();
    // A person's gestures are the screen's work: the visible lane, ahead
    // of what prefetches.
    let kept = job.clone();
    let called = match &job {
        Job::Call { tool, args, .. } => Some((tool.clone(), args.clone())),
        _ => None,
    };
    let refreshing = matches!(job, Job::Refresh);
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
            // A read that panics never catches up, and would ask again for ever.
            let again = done.is_ok() || !refreshing;
            match done {
                Ok(done) => finish(engine, ws, done),
                Err(panic) => {
                    let message = panic.message().to_owned();
                    let _ = engine.set(ws, Workspace::ERROR, message.clone());
                    // Whoever asked is still waiting for the answer.
                    if let Some((tool, args)) = called {
                        let error = json!({"code": "internal", "message": message});
                        let _ = Workspace::emit_refused(&Cx::new(engine, ws), tool, error, args);
                    }
                }
            }
            if again {
                refresh_if_behind(engine, ws);
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
        Job::Call {
            tool,
            args,
            follows,
        } => {
            let answer = server
                .call(&tool, &person(), args.clone())
                .map(|reply| reply.structured)
                .map_err(|e| refusal(e.to_value()));
            // Reading where things are changes nothing a panel shows.
            let view = (!is_geometry(&tool, &args)).then(read);
            Done::Call {
                tool,
                args,
                follows,
                answer,
                view,
            }
        }
        Job::Refresh => {
            let (view, seen) = read();
            Done::Refresh { view, seen }
        }
        Job::Picture { generation, looks } => {
            let args = json!({"format": "png", "max_size": 1280});
            let picture = server
                .shown_picture(&person(), args)
                .map_err(|e| e.to_string())
                .and_then(|reply| {
                    let image = reply
                        .images
                        .into_iter()
                        .find(|i| i.mime == "image/png")
                        .ok_or_else(|| "the preview answered no PNG".to_owned())?;
                    // Here, not where it lands: hashing a window's pixels is
                    // milliseconds the engine's thread has no use for.
                    let (w, h, rgba) = decode(&image.data)?;
                    let pixels = hash_of(&rgba);
                    Ok((w, h, rgba, pixels))
                });
            Done::Picture {
                generation,
                looks,
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

fn is_geometry(tool: &str, args: &Json) -> bool {
    tool == "node_arrange" && args["action"] == "geometry"
}

/// Publishes what a job did, on the UI thread.
fn finish(engine: &Engine, ws: ObjectId, done: Done) {
    let cx = Cx::new(engine, ws);
    match done {
        Done::Call {
            tool,
            args,
            follows,
            answer,
            view,
        } => {
            if let Ok(answer) = &answer {
                rebase(engine, ws, &tool, &args, answer);
            }
            if let Some((view, seen)) = view {
                publish(engine, ws, view, seen);
            }
            // What the canvas reads for itself is no gesture of the person's:
            // it neither clears the refusal of the drop before it nor raises one.
            // The export panel shows its own answers and refusals. A call made
            // with `follow` must not clear or replace the refusal of the gesture
            // it follows; with none on the line its own is the only word the
            // person gets.
            let gesture = !(is_geometry(&tool, &args) || tool == "export" || follows);
            match answer {
                Ok(answer) => {
                    if gesture {
                        let _ = cx.set(Workspace::ERROR, String::new());
                    }
                    let _ = Workspace::emit_answered(&cx, tool, answer, args);
                }
                Err(error) => {
                    let free = || {
                        engine
                            .get(ws, Workspace::ERROR)
                            .unwrap_or_default()
                            .is_empty()
                    };
                    if gesture || (follows && free()) {
                        let message = error["message"].as_str().unwrap_or_default().to_owned();
                        let _ = cx.set(Workspace::ERROR, message);
                    }
                    let _ = Workspace::emit_refused(&cx, tool, error, args);
                }
            }
        }
        Done::Refresh { view, seen } => publish(engine, ws, view, seen),
        Done::Picture {
            generation,
            looks,
            picture,
        } => {
            let Some(queue) = queue_of(engine, ws) else {
                return;
            };
            for looked in looks {
                let _ = looked.send(());
            }
            let picture = picture.and_then(|(w, h, rgba, pixels)| {
                let image = ImageHandle::from_straight(w, h, rgba).map_err(|e| e.to_string())?;
                Ok((image, pixels))
            });
            let failed = picture.is_err();
            let mut changed = false;
            match picture {
                Ok((image, pixels)) => {
                    {
                        let mut q = queue.borrow_mut();
                        changed = q.pixels != pixels || q.picture != generation;
                        if changed {
                            q.image = Some(image);
                            q.picture = generation;
                            q.pixels = pixels;
                            q.shots += 1;
                        }
                    }
                    let mut preview = engine.get(ws, Workspace::PREVIEW).unwrap_or_default();
                    if let Some(fields) = preview.as_object_mut()
                        && changed
                    {
                        fields.insert("picture".to_owned(), json!(generation));
                        fields.insert("shots".to_owned(), json!(queue.borrow().shots));
                        let _ = cx.set(Workspace::PREVIEW, preview);
                    }
                }
                Err(e) => {
                    let _ = cx.set(Workspace::ERROR, format!("no picture of the preview: {e}"));
                }
            }
            let looks = {
                let mut q = queue.borrow_mut();
                q.looks += 1;
                q.looks
            };
            let _ = cx.set(Workspace::LOOKS, looks);
            let mut q = queue.borrow_mut();
            if let Some(settling) = &mut q.settling {
                settling.same = if changed { 0 } else { settling.same + 1 };
                settling.looks_left = settling.looks_left.saturating_sub(1);
                if failed || settling.same >= Settling::SAME || settling.looks_left == 0 {
                    q.settling = None;
                    drop(q);
                    let _ = cx.set(Workspace::SETTLING, false);
                }
            }
        }
    }
}

/// How long one wait on the preview lasts; a stop ends it earlier.
const FOLLOW_WAIT_MS: u64 = 30_000;
const LOOK_AT_MOST_EVERY_MS: u64 = 100;

/// Ends a following thread's wait on the preview as soon as it is told, not
/// when the wait runs out: for an app at rest that is half a minute, in which
/// the thread would hold the session, and the preview's connection, for
/// nobody.
struct Stop {
    stopped: AtomicBool,
    /// Keeps a stop told before the wait began, so it is not missed.
    woken: tokio::sync::Notify,
}

impl Stop {
    fn new() -> Arc<Self> {
        Arc::new(Stop {
            stopped: AtomicBool::new(false),
            woken: tokio::sync::Notify::new(),
        })
    }

    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.woken.notify_one();
    }

    fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}

/// `preview_events` waiting `args`, dropped unanswered if `stop` is told
/// first; `None` then, and when it could not be asked.
fn wait_on_preview(
    runtime: &tokio::runtime::Runtime,
    weak: &std::sync::Weak<StudioServer>,
    args: Json,
    stop: &Stop,
) -> Option<Json> {
    let server = weak.upgrade()?;
    let ctx = person();
    runtime.block_on(async {
        tokio::select! {
            reply = server.call_tool("preview_events", &ctx, args) => {
                reply.ok().map(|reply| reply.structured)
            }
            () = stop.woken.notified() => None,
        }
    })
}

/// Starts waiting on the preview, if wanted and not already waiting. Nothing
/// polls: the wait answers when the app has drawn.
fn follow(engine: &Engine, ws: ObjectId) {
    let Some(queue) = queue_of(engine, ws) else {
        return;
    };
    let context = engine.extension::<Context>();
    // It may not keep the session, and so the preview, past the window: it
    // holds the server only while it asks.
    let Some(weak) = context.server.borrow().as_ref().map(Arc::downgrade) else {
        return;
    };
    let stop = Stop::new();
    {
        let mut q = queue.borrow_mut();
        if !q.follow_wanted || q.following.is_some() {
            return;
        }
        q.following = Some(Arc::clone(&stop));
    }
    let post = Platform::of(engine).poster();
    let mine = Arc::clone(&stop);
    let spawned = std::thread::Builder::new()
        .name("studio-picture-watch".to_owned())
        .spawn(move || {
            // A wait here is dropped when told to stop, which needs a
            // reactor of its own to be driven on.
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok();
            let mut since = None;
            while let Some(runtime) = runtime.as_ref().filter(|_| !stop.is_stopped()) {
                let mut args = json!({"wait_ms": FOLLOW_WAIT_MS});
                if let Some(since) = since {
                    args["rests"] = json!(since);
                }
                let Some(reply) = wait_on_preview(runtime, &weak, args, &stop) else {
                    break;
                };
                let held = since;
                since = reply["rests"].as_u64();
                // A rest told of past one already held is a change, and the
                // last of them, an animation's end, has no wake after it.
                let moved = held.is_some() && since != held;
                // One that will wake is looked at when it has, and the look
                // makes it rest again with its next wake: this asks only as
                // often as the app itself moves, and no more than a person
                // can see. The wait is on the app: a new rest ends it early.
                if let Some(wake) = reply["wake_in_ms"].as_u64() {
                    std::thread::sleep(Duration::from_millis(LOOK_AT_MOST_EVERY_MS));
                    let rest = wake
                        .min(FOLLOW_WAIT_MS)
                        .saturating_sub(LOOK_AT_MOST_EVERY_MS);
                    if rest > 0 && !stop.is_stopped() {
                        let mut args = json!({"wait_ms": rest});
                        if let Some(since) = since {
                            args["rests"] = json!(since);
                        }
                        let Some(reply) = wait_on_preview(runtime, &weak, args, &stop) else {
                            break;
                        };
                        since = reply["rests"].as_u64();
                    }
                } else if !moved {
                    // An app that will not wake by itself shows what it shows.
                    continue;
                }
                if !stop.is_stopped() {
                    // The next look's wait starts when this picture has
                    // landed: a picture slower than the app moves would
                    // otherwise have the next asked before it is done, and
                    // the lane would never be free between them. Without
                    // a Workspace nothing holds the sender, and this ends.
                    let (landed, wait) = mpsc::channel();
                    post.send(move |engine| look(engine, ws, landed));
                    let _ = wait.recv_timeout(Duration::from_millis(FOLLOW_WAIT_MS));
                }
            }
            // No preview to wait on: the next one's refresh starts this again.
            post.send(move |engine| {
                if let Some(queue) = queue_of(engine, ws) {
                    let mut q = queue.borrow_mut();
                    if q.following.as_ref().is_some_and(|s| Arc::ptr_eq(s, &stop)) {
                        q.following = None;
                    }
                }
            });
        });
    if spawned.is_err() {
        // No thread to wait on: the next refresh tries again.
        let mut q = queue.borrow_mut();
        if q.following.as_ref().is_some_and(|s| Arc::ptr_eq(s, &mine)) {
            q.following = None;
        }
    }
}

/// Asks for the picture again, unless an ask is already waiting.
fn look(engine: &Engine, ws: ObjectId, landed: mpsc::Sender<()>) {
    if !engine.is_alive(ws) {
        return;
    }
    let Some(queue) = queue_of(engine, ws) else {
        return;
    };
    let generation = {
        let mut q = queue.borrow_mut();
        let waiting = q.jobs.iter_mut().find_map(|j| match j {
            Job::Picture { looks, .. } => Some(looks),
            _ => None,
        });
        match waiting {
            Some(looks) => {
                looks.push(landed);
                return;
            }
            None => q.fetched,
        }
    };
    enqueue(
        engine,
        ws,
        Job::Picture {
            generation,
            looks: vec![landed],
        },
    );
}

fn hash_of(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(bytes);
    hasher.finish()
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
        if let Job::Call {
            tool, args: next, ..
        } = job
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
    let (picture, fetched, shots) = {
        let mut q = queue.borrow_mut();
        q.published = q.published.max(seen);
        (q.picture, q.fetched, q.shots)
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
            "shots": shots,
        }),
        _ => Json::Null,
    };
    let size = view
        .preview
        .as_ref()
        .map_or((0, 0), |p| (p.width, p.height));
    let resized = queue_of(engine, ws)
        .is_some_and(|q| std::mem::replace(&mut q.borrow_mut().size, size) != size);
    let other_doc = queue_of(engine, ws).is_some_and(|q| {
        let doc = view.preview.as_ref().map_or("", |p| p.doc.as_str());
        let mut q = q.borrow_mut();
        let other = q.previewed != doc;
        if other {
            doc.clone_into(&mut q.previewed);
        }
        other
    });
    let wanted = view
        .preview
        .as_ref()
        .filter(|p| p.doc == view.doc)
        .map(|p| p.generation)
        .filter(|&g| resized || other_doc || (g != picture && g != fetched));
    let _ = cx.set(Workspace::BLANK, view.blank);
    give(
        &cx,
        Workspace::DOCUMENTS,
        "DOCUMENTS",
        to_json(&view.documents),
        &queue,
    );
    give(
        &cx,
        Workspace::ASSETS,
        "ASSETS",
        to_json(&view.assets),
        &queue,
    );
    let _ = cx.set(Workspace::GENERATION, view.generation as f64);
    let _ = cx.set(Workspace::TEXT, view.text);
    give(
        &cx,
        Workspace::LAYERS,
        "LAYERS",
        to_json(&view.layers),
        &queue,
    );
    let _ = cx.set(Workspace::SELECTED, view.selected);
    give(
        &cx,
        Workspace::CHOSEN,
        "CHOSEN",
        to_json(&view.chosen),
        &queue,
    );
    give(
        &cx,
        Workspace::SELECTION,
        "SELECTION",
        to_json(&view.selection),
        &queue,
    );
    give(
        &cx,
        Workspace::INSPECTOR,
        "INSPECTOR",
        to_json(&view.inspector),
        &queue,
    );
    give(
        &cx,
        Workspace::STATES,
        "STATES",
        to_json(&view.states),
        &queue,
    );
    give(&cx, Workspace::ROLES, "ROLES", to_json(&view.roles), &queue);
    give(
        &cx,
        Workspace::SIGNALS,
        "SIGNALS",
        to_json(&view.signals),
        &queue,
    );
    give(
        &cx,
        Workspace::OFFERS,
        "OFFERS",
        to_json(&view.offers),
        &queue,
    );
    give(&cx, Workspace::FLOW, "FLOW", to_json(&view.flow), &queue);
    give(
        &cx,
        Workspace::PALETTE,
        "PALETTE",
        to_json(&view.palette),
        &queue,
    );
    give(
        &cx,
        Workspace::TOOLKIT,
        "TOOLKIT",
        to_json(&view.toolkit),
        &queue,
    );
    give(
        &cx,
        Workspace::ROOT_STATES,
        "ROOT_STATES",
        to_json(&view.root_states),
        &queue,
    );
    give(&cx, Workspace::DONE, "DONE", to_json(&view.done), &queue);
    give(
        &cx,
        Workspace::UNDONE,
        "UNDONE",
        to_json(&view.undone),
        &queue,
    );
    give(
        &cx,
        Workspace::LEASES,
        "LEASES",
        to_json(&view.leases),
        &queue,
    );
    if let Some(ms) = view.leases.iter().map(|l| l.ends_in_ms).min() {
        wake_after(engine, Duration::from_millis(ms));
    }
    give(
        &cx,
        Workspace::COMMENTS,
        "COMMENTS",
        to_json(&view.comments),
        &queue,
    );
    let _ = cx.set(Workspace::PREVIEW, preview);
    let _ = cx.set(Workspace::PROBLEM, view.problem);
    let _ = cx.set(Workspace::PREVIEWS, view.previews);
    let _ = cx.set(Workspace::THEME, view.theme);
    give(
        &cx,
        Workspace::TOKENS,
        "TOKENS",
        to_json(&view.tokens),
        &queue,
    );
    // A click changes what the app shows and no generation says so. A new
    // preview counts from nothing again, which is no click.
    let clicks = view
        .preview
        .as_ref()
        .filter(|p| p.doc == view.doc)
        .map_or(0, |p| p.clicks);
    let clicked = {
        let mut q = queue.borrow_mut();
        let clicked = clicks > q.clicks;
        q.clicks = clicks;
        if clicked {
            q.settling = Some(Settling {
                looks_left: Settling::LOOKS,
                same: 0,
            });
        }
        clicked
    };
    if clicked {
        let _ = cx.set(Workspace::SETTLING, true);
    }
    follow(engine, ws);
    // A picture is fetched once for each generation the preview applies.
    if let Some(generation) = wanted {
        let queued = queue
            .borrow()
            .jobs
            .iter()
            .any(|j| matches!(j, Job::Picture { generation: g, .. } if *g == generation));
        if !queued {
            queue.borrow_mut().fetched = generation;
            enqueue(
                engine,
                ws,
                Job::Picture {
                    generation,
                    looks: Vec::new(),
                },
            );
        }
    }
}

/// A lease ends without a write or a call to tell of it: every workspace
/// reads the session again once `after` went by. One wake waits at a
/// time, the earliest; a later one is asked for again by the refresh it
/// brings.
fn wake_after(engine: &Engine, after: Duration) {
    let context = engine.extension::<Context>();
    let at = Instant::now() + after;
    if context.wake_at.get().is_some_and(|w| w <= at) {
        return;
    }
    context.wake_at.set(Some(at));
    let post = Platform::of(engine).poster();
    let spawned = std::thread::Builder::new()
        .name("studio-lease-wake".to_owned())
        .spawn(move || {
            std::thread::sleep(after);
            post.send(move |engine| {
                let context = engine.extension::<Context>();
                if context.wake_at.get() != Some(at) {
                    return;
                }
                context.wake_at.set(None);
                let live: Vec<ObjectId> = {
                    let mut all = context.workspaces.borrow_mut();
                    all.retain(|&ws| engine.is_alive(ws));
                    all.clone()
                };
                for ws in live {
                    enqueue(engine, ws, Job::Refresh);
                }
            });
        });
    if spawned.is_err() {
        // No thread to wait on: the label stays until the next write or
        // gesture reads the session again.
        context.wake_at.set(None);
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
        // A job running or waiting may be what wrote, and reads after it: it
        // says when it is done whether that was enough.
        let (busy, empty) = queue_of(engine, ws).map_or((true, false), |q| {
            let q = q.borrow();
            (q.busy, q.jobs.is_empty())
        });
        if !busy && empty {
            refresh_if_behind_of(engine, ws, writes);
        } else if !busy {
            // Jobs that could not start: this is the next write to ask again.
            pump(engine, ws);
        }
    }
}

fn refresh_if_behind(engine: &Engine, ws: ObjectId) {
    let writes = engine.extension::<Context>().writes.load(Ordering::SeqCst);
    refresh_if_behind_of(engine, ws, writes);
}

fn refresh_if_behind_of(engine: &Engine, ws: ObjectId, writes: u64) {
    let behind = queue_of(engine, ws).is_some_and(|q| q.borrow().published < writes);
    if behind {
        enqueue(engine, ws, Job::Refresh);
    }
}

fn lock(
    studio: &std::sync::Mutex<studio_mcp::Studio>,
) -> std::sync::MutexGuard<'_, studio_mcp::Studio> {
    studio
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Sets a list a panel shows, if it is not what the panel has: a panel over a
/// list made anew makes its rows anew, and most publishes change one list.
fn give(cx: &Cx, prop: Prop<Json>, name: &'static str, value: Json, queue: &Rc<RefCell<Queue>>) {
    let mut q = queue.borrow_mut();
    if q.given.get(name) != Some(&value) {
        q.given.insert(name, value.clone());
        drop(q);
        let _ = cx.set(prop, value);
    }
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
