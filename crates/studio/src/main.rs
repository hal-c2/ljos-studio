//! `studio`: Ljós Studio.
//!
//! `studio [DIR]` opens Studio's window on the project in DIR (the current
//! directory when left out), and serves the same tools to agents on a
//! loopback endpoint while it is open. `studio --mcp [DIR]` serves them on
//! stdio instead, with no window.

use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::Parser;
use ljos_mcp_core::{Cap, CapSet};
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

#[derive(Parser, Debug)]
#[command(name = "studio", version, about)]
struct Args {
    /// Serve Studio's MCP tools on stdin and stdout, with no window.
    #[arg(long)]
    mcp: bool,
    /// What an agent may do: a comma-separated list of `inspect`,
    /// `mutate`, `fs` and `run`.
    #[arg(long, default_value = "inspect,mutate")]
    caps: String,
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
    let caps = CapSet::parse_list(&args.caps)?;
    // Reading is what every other tool builds on.
    let caps = caps.with(Cap::Inspect);
    let dir = match args.dir {
        Some(dir) => dir,
        None => std::env::current_dir().context("the current directory")?,
    };
    let ljos = args.ljos.unwrap_or_else(NativeHost::default_program);
    let state = state_dir();
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
/// window's own token file is: a directory only this user can read (a token
/// grants control), and outside the project, which the preview watches. One
/// level, by process: a shared parent in a shared temporary directory would
/// let one other user stop every Studio's previews.
fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    base.join(format!("ljos-studio-{}", std::process::id()))
}
