//! `studio`: Ljós Studio.
//!
//! `studio --mcp [DIR]` serves Studio's tools on stdio for an agent, on the
//! project in DIR (the current directory when left out). The canvas comes
//! with milestone S3; until then the tools are the whole of Studio.

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, bail};
use clap::Parser;
use ljos_mcp_core::{Cap, CapSet};
use studio_mcp::{Studio, StudioServer};
use studio_preview::NativeHost;

#[derive(Parser, Debug)]
#[command(name = "studio", version, about)]
struct Args {
    /// Serve Studio's MCP tools on stdin and stdout.
    #[arg(long)]
    mcp: bool,
    /// What the agent may do: a comma-separated list of `inspect`,
    /// `mutate`, `fs` and `run`.
    #[arg(long, default_value = "inspect,mutate")]
    caps: String,
    /// The `ljos` program previews run (default: `LJOS`, else `ljos` on
    /// the `PATH`).
    #[arg(long)]
    ljos: Option<PathBuf>,
    /// The workspace: the project opened first, and the directory
    /// `project_open` may open others below.
    dir: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.mcp {
        bail!("the canvas is not built yet: run `studio --mcp [DIR]` to serve the tools");
    }
    let caps = CapSet::parse_list(&args.caps)?;
    // Reading is what every other tool builds on.
    let caps = caps.with(Cap::Inspect);
    let dir = match args.dir {
        Some(dir) => dir,
        None => std::env::current_dir().context("the current directory")?,
    };
    let ljos = args.ljos.unwrap_or_else(NativeHost::default_program);
    let state = preview_state_dir();
    let studio = Studio::open(&dir)
        .with_context(|| format!("opening {}", dir.display()))?
        .with_previews(Arc::new(move |project| {
            Box::new(NativeHost::new(project, &ljos, &state))
        }));
    let server = StudioServer::new(studio);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(studio_mcp::serve_stdio(server, caps))?;
    Ok(())
}

/// Where previews keep their inspector token files and logs: a directory
/// only this user can read (the token grants control of the preview), and
/// outside the project, which the preview watches.
fn preview_state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    base.join("ljos-studio")
        .join(std::process::id().to_string())
}
