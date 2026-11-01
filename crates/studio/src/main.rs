//! `studio`: Ljós Studio.
//!
//! `studio [DIR]` opens Studio's window on the project in DIR (the current
//! directory when left out), and serves the same tools to agents on a
//! loopback endpoint while it is open. `studio --mcp [DIR]` serves them on
//! stdio instead, with no window.
//!
//! The window shows the person's own shell when they have one: the
//! directory `--shell DIR` names, else `ljos-studio/shell` in their
//! configuration directory when it is there. That shell is watched, so an
//! edit of it, or of Studio's theme file, reskins the window as it runs;
//! one that does not work gives the window back to the shell Studio ships,
//! with a banner saying why, until it is mended.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use clap::Parser;
use ljos_mcp_core::{Cap, CapSet};
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

mod state;

#[derive(Parser, Debug)]
#[command(name = "studio", version, about)]
struct Args {
    /// Serve Studio's MCP tools on stdin and stdout, with no window.
    #[arg(long)]
    mcp: bool,
    /// What an agent may do: a comma-separated list of `inspect`,
    /// `mutate`, `fs` and `run`. By default an agent on the window's endpoint
    /// may read, and the client that started `--mcp` may also edit.
    #[arg(long)]
    caps: Option<String>,
    /// The `ljos` program previews run (default: `LJOS`, else `ljos` on
    /// the `PATH`).
    #[arg(long)]
    ljos: Option<PathBuf>,
    /// A shell of your own: a directory of QML whose `Shell.qml` the
    /// window shows in place of the one Studio ships, watched as it runs.
    /// A directory without one is given a copy of Studio's to start from.
    /// Default: `ljos-studio/shell` in the configuration directory, when
    /// it is there.
    #[arg(long, value_name = "DIR", conflicts_with = "mcp")]
    shell: Option<PathBuf>,
    /// The roles of `Ljos.Design` Studio's own window is drawn with, a
    /// theme file as `theme_set` writes one. Default:
    /// `ljos-studio/theme.json` in the configuration directory.
    #[arg(long, value_name = "FILE", conflicts_with = "mcp")]
    theme: Option<PathBuf>,
    /// Serve no tools to agents while the window is open.
    #[arg(long, conflicts_with = "mcp")]
    no_agents: bool,
    /// The workspace: the project opened first, and the directory
    /// `project_open` may open others below.
    dir: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let caps = granted(&args)?;
    let dir = match args.dir {
        Some(dir) => dir,
        None => std::env::current_dir().context("the current directory")?,
    };
    let ljos = args.ljos.unwrap_or_else(NativeHost::default_program);
    // Dropped last, on every way out but a kill: the next start removes
    // what a killed run left (`state.rs`).
    let state_dir = state_dir()?;
    let state = state_dir.path().to_owned();
    let previews = state.clone();
    let studio = Studio::open(&dir)
        .with_context(|| format!("opening {}", dir.display()))?
        .with_previews(Arc::new(move |project| {
            Box::new(NativeHost::new(project, &ljos, &previews))
        }));
    stop_on_signal(studio.preview_closer(), state.clone());
    // The window's end ends the preview, whoever still holds the session: a
    // process leaving runs no destructor another thread's Arc keeps alive.
    let close_preview = studio.preview_closer();
    let server = Arc::new(StudioServer::new(studio));
    if args.mcp {
        let server = Arc::into_inner(server).expect("nothing else holds the server yet");
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(studio_mcp::serve_stdio(server, caps))?;
        return Ok(());
    }
    // Held until the window closes; dropping it stops the endpoint and
    // removes its token file.
    let _agents = if args.no_agents {
        None
    } else {
        let http = studio_mcp::endpoint::serve_agents(&server, caps, &state)
            .context("serving Studio's tools on loopback")?;
        eprintln!(
            "studio: agents connect at {} with the token in {}",
            http.url(),
            state.join(studio_mcp::endpoint::TOKEN_FILE).display()
        );
        Some(http)
    };
    let config = config_dir();
    let shell = match args.shell {
        Some(dir) => {
            seed(&dir)?;
            Some(dir)
        }
        None => config
            .as_ref()
            .map(|c| c.join("shell"))
            .filter(|d| d.join("Shell.qml").is_file()),
    };
    let theme = args.theme.or_else(|| config.map(|c| c.join("theme.json")));
    // A watched window watches where the theme file would be, and a first run
    // has no such directory yet.
    if let (Some(_), Some(dir)) = (&shell, theme.as_deref().and_then(Path::parent)) {
        std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    let ended = match shell {
        Some(shell) => watched_window(server, &shell, theme, &state),
        None => window(server, theme),
    };
    close_preview();
    ended
}

/// A signal skips every `Drop`, which is what closes a preview, so a stopped
/// Studio would leave its `ljos` running. A SIGKILL still does: only the child
/// can notice that (Ljós, request 4).
fn stop_on_signal(close: impl FnOnce() + Send + 'static, state: PathBuf) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build();
        let signal = match runtime.map(|r| r.block_on(stop_asked())) {
            Ok(Ok(signal)) => signal,
            Ok(Err(e)) | Err(e) => {
                eprintln!("studio: signals will not close a preview: {e}");
                return;
            }
        };
        // Bounded: asked to stop, Studio stops, whatever a preview is doing.
        let (closed, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            close();
            let _ = closed.send(());
        });
        let _ = wait.recv_timeout(std::time::Duration::from_secs(2));
        let _ = std::fs::remove_dir_all(&state);
        end(128 + signal);
    });
}

/// Ends the process without exit handlers: they would run here while the
/// window's thread still draws with what they free, as a GL driver's did.
#[cfg(unix)]
fn end(code: i32) -> ! {
    signal_hook::low_level::exit(code)
}

#[cfg(not(unix))]
fn end(code: i32) -> ! {
    std::process::exit(code)
}

/// Waits for a request to stop, and says which signal it was.
#[cfg(unix)]
async fn stop_asked() -> std::io::Result<i32> {
    use tokio::signal::unix::{SignalKind, signal};
    let (term, int) = (SignalKind::terminate(), SignalKind::interrupt());
    let (mut terms, mut ints) = (signal(term)?, signal(int)?);
    tokio::select! {
        _ = terms.recv() => Ok(term.as_raw_value()),
        _ = ints.recv() => Ok(int.as_raw_value()),
    }
}

#[cfg(not(unix))]
async fn stop_asked() -> std::io::Result<i32> {
    tokio::signal::ctrl_c().await?;
    Ok(2)
}

/// What a client may do. The endpoint can be reached by whatever else runs on
/// this machine and finds the token, so more than reading is the person's to
/// grant (DESIGN.md); a client on stdio was started for the purpose.
fn granted(args: &Args) -> anyhow::Result<CapSet> {
    let default = if args.mcp {
        "inspect,mutate"
    } else {
        "inspect"
    };
    let caps = CapSet::parse_list(args.caps.as_deref().unwrap_or(default))?;
    // Reading is what every other tool builds on.
    Ok(caps.with(Cap::Inspect))
}

/// The window on the shell Studio ships, compiled ahead of time, drawn
/// with `theme` as the file is when the window opens.
fn window(server: Arc<StudioServer>, theme: Option<PathBuf>) -> anyhow::Result<()> {
    let theme = match theme {
        Some(file) => read_theme(&file)?,
        None => None,
    };
    ljos::App::new("Ljós Studio")
        .size(1440.0, 900.0)
        .register(studio_blocks::register)
        .source(ljos::include_qml!("shell.pack"))
        .compiled(ljos::include_compiled!("shell.rs"))
        .root("Window")
        .run_with(move |setup| {
            if let Some(theme) = &theme {
                ljos_design_qml::theme::apply(setup.engine(), Some(theme))
                    .map_err(|errors| -> ljos::SetupError { joined(&errors).into() })?;
            }
            // Before the root is made, so its workspace starts with the session.
            studio_blocks::serve(setup.engine(), server);
            setup
                .engine()
                .create_from_url("Window", None, Vec::new())
                .map_err(|e| -> ljos::SetupError { e.to_string().into() })
        })?;
    Ok(())
}

/// The window on the person's `shell`, with the one Studio ships beneath
/// it, both watched with Studio's `theme`: a save is applied as the
/// window runs, and one that fails says why on the window's banner.
fn watched_window(
    server: Arc<StudioServer>,
    shell: &Path,
    theme: Option<PathBuf>,
    state: &Path,
) -> anyhow::Result<()> {
    // The watched session compiles from the disk, so Studio's own shell is
    // written out, in the state directory that goes with the process.
    let builtin = state.join("shell");
    studio_blocks::shell::write(&builtin)
        .with_context(|| format!("writing Studio's shell to {}", builtin.display()))?;
    let user_shell = studio_blocks::shell::url(&shell.join("Shell.qml"))?;
    let shell = shell.to_owned();
    ljos::App::new("Ljós Studio")
        .size(1440.0, 900.0)
        .register(studio_blocks::register)
        .run_with(move |setup| {
            studio_blocks::serve(setup.engine(), server);
            if let Some(theme) = theme {
                setup.set_theme_file(theme);
            }
            let engine = std::rc::Rc::clone(setup.engine());
            setup.on_reload(move |report| reported(&engine, report));
            let oracle = ljos::dev::EngineOracle::from_engine(setup.engine());
            let made = setup.watch_roots_with(
                &[&shell, &builtin],
                &["Window"],
                vec![("userShell".to_owned(), user_shell.as_str().into())],
                oracle,
            )?;
            // A theme that does not load as the window opens sends no
            // report, but Ljós holds its error.
            studio_blocks::reload_settled(setup.engine());
            Ok(made)
        })?;
    Ok(())
}

/// Tells the window why a save of the shell or the theme did not apply, or
/// that none stands: a theme that loads after one that did not is a
/// `Theme` report, not a `Cleared` one.
fn reported(engine: &ljos::Engine, report: &ljos::reload::Report) {
    use ljos::reload::Report;
    match report {
        Report::Failed { .. } | Report::Reloaded(..) | Report::Cleared | Report::Theme { .. } => {
            studio_blocks::reload_settled(engine);
        }
        _ => {}
    }
}

/// Gives `dir` a copy of Studio's shell when it has no `Shell.qml`: a
/// person's shell starts as Studio's, to be changed.
fn seed(dir: &Path) -> anyhow::Result<()> {
    if dir.join("Shell.qml").is_file() {
        return Ok(());
    }
    studio_blocks::shell::seed(dir)
        .with_context(|| format!("copying Studio's shell to {}", dir.display()))?;
    eprintln!(
        "studio: {} had no Shell.qml; it now has a copy of Studio's",
        dir.display()
    );
    Ok(())
}

/// Studio's own theme from `file`; none when there is no such file.
fn read_theme(file: &Path) -> anyhow::Result<Option<ljos_design::theme::Theme>> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
    };
    ljos_design::theme::parse(&file.to_string_lossy(), &text)
        .map(Some)
        .map_err(|errors| anyhow::anyhow!(joined(&errors)))
}

fn joined(errors: &[ljos_design::theme::ThemeError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Where a person's own shell and theme for Studio are.
fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("ljos-studio"))
}

/// Where previews keep their inspector token files and logs, and where the
/// window's own token file is: outside the project, which the preview
/// watches, and only this user's, as a token grants control: made so before
/// the window's token is written, and refused if found otherwise. Never below
/// the shared temporary directory, where another user could have made it
/// first; one level, by process, so no shared parent of ours is there to take.
/// Removed when the run ends, as below `XDG_STATE_HOME` nothing else would.
fn state_dir() -> anyhow::Result<state::StateDir> {
    let base = state_base(
        std::env::var_os("XDG_RUNTIME_DIR"),
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")),
    )
    .context("no place for Studio's state: set XDG_RUNTIME_DIR, XDG_STATE_HOME or HOME")?;
    state::StateDir::new(&base)
}

fn state_base(
    runtime: Option<OsString>,
    state: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let set = |dir: Option<OsString>| dir.filter(|d| !d.is_empty()).map(PathBuf::from);
    set(runtime)
        .or_else(|| set(state))
        .or_else(|| set(home).map(|h| h.join(".local").join("state")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn granted_by(argv: &[&str]) -> CapSet {
        granted(&Args::parse_from(std::iter::once(&"studio").chain(argv))).unwrap()
    }

    #[test]
    fn the_windows_endpoint_may_read_unless_the_person_grants_more() {
        assert_eq!(granted_by(&[]), CapSet::of([Cap::Inspect]));
        assert_eq!(
            granted_by(&["--caps", "mutate"]),
            CapSet::of([Cap::Inspect, Cap::Mutate])
        );
    }

    #[test]
    fn a_client_on_stdio_may_edit_as_it_always_could() {
        assert_eq!(
            granted_by(&["--mcp"]),
            CapSet::of([Cap::Inspect, Cap::Mutate])
        );
    }

    #[test]
    fn the_state_is_never_in_the_shared_temporary_directory() {
        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(
            state_base(os("/run/user/1"), os("/s"), os("/h")),
            Some(PathBuf::from("/run/user/1"))
        );
        assert_eq!(
            state_base(os(""), os("/s"), os("/h")),
            Some(PathBuf::from("/s"))
        );
        assert_eq!(
            state_base(None, None, os("/h")),
            Some(PathBuf::from("/h/.local/state"))
        );
        assert_eq!(state_base(None, None, None), None);
    }
}
