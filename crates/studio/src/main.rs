//! `studio`: Ljós Studio.
//!
//! `studio [DIR]` opens Studio's window on the project in DIR (the current
//! directory when left out), and serves the same tools to agents on a
//! loopback endpoint while it is open. `studio --mcp [DIR]` serves them on
//! stdio instead, with no window.

use std::{ffi::OsString, path::PathBuf, sync::Arc};

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
    /// A shell of your own: a directory of QML whose `Main.qml` is the
    /// window, compiled at start in place of the one Studio ships.
    #[arg(long, value_name = "DIR", conflicts_with = "mcp")]
    shell: Option<PathBuf>,
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
    window(server, args.shell)
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

/// Opens the window and runs until it is closed.
fn window(server: Arc<StudioServer>, shell: Option<PathBuf>) -> anyhow::Result<()> {
    let app = ljos::App::new("Ljós Studio")
        .size(1440.0, 900.0)
        .register(studio_blocks::register);
    let app = match shell {
        Some(dir) => app
            .qml_dir(&dir, "Main")
            .with_context(|| format!("compiling the shell in {}", dir.display()))?,
        None => app
            .source(ljos::include_qml!("shell.pack"))
            .compiled(ljos::include_compiled!("shell.rs"))
            .root("Main"),
    };
    app.run_with(move |setup| {
        // Before the root is made, so its workspace starts with the session.
        studio_blocks::serve(setup.engine(), server);
        setup
            .engine()
            .create_from_url("Main", None, Vec::new())
            .map_err(|e| -> ljos::SetupError { e.to_string().into() })
    })?;
    Ok(())
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
