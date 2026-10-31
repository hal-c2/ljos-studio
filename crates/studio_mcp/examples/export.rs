//! Calls `export` on a project as an agent does and prints the answer, for
//! tasks that need a bundle without an agent (`mise run export:web:check`).
//!
//! `cargo run -p studio_mcp --example export -- DIR ENTRY [native|web]`

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::json;
use studio_mcp::{Studio, StudioServer};

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: export DIR ENTRY [native|web]";
    let dir = args.next().expect(usage);
    let entry = args.next().expect(usage);
    let target = args.next().unwrap_or_else(|| "native".to_owned());
    let server = StudioServer::new(Studio::open(&dir).unwrap_or_else(|e| panic!("{dir}: {e}")));
    let ctx = CallCtx::new(CapSet::default().with(Cap::Inspect).with(Cap::Fs));
    let (out, _) = server
        .tools()
        .call("export", &ctx, json!({ "entry": entry, "target": target }));
    match out {
        Ok(out) => println!("{}", serde_json::to_string_pretty(&out).expect("JSON")),
        Err(e) => {
            eprintln!("{}", e.to_value());
            std::process::exit(1);
        }
    }
}
