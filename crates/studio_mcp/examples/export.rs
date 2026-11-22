//! Calls `export` on a project as an agent does and prints the answer, for
//! tasks that need a bundle without an agent (`mise run export:web:check`).
//!
//! `cargo run -p studio_mcp --example export -- DIR ENTRY [native|web] [--build]`
//!
//! `--build` builds the site as Studio's window does, with the ljos-web and
//! `qmlc` that `LJOS_WEB` and `QMLC` name, or that Studio finds.

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::json;
use studio_mcp::{Studio, StudioServer};

fn main() {
    let mut args = std::env::args().skip(1);
    let usage = "usage: export DIR ENTRY [native|web] [--build]";
    let dir = args.next().expect(usage);
    let entry = args.next().expect(usage);
    let rest: Vec<String> = args.collect();
    let build = rest.iter().any(|a| a == "--build");
    let target = rest
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "native".to_owned());
    let server = StudioServer::new(Studio::open(&dir).unwrap_or_else(|e| panic!("{dir}: {e}")));
    let mut caps = CapSet::default().with(Cap::Inspect).with(Cap::Fs);
    let mut call = json!({ "entry": entry, "target": target });
    if build {
        caps = caps.with(Cap::Run);
        call["build"] = json!(true);
    }
    let ctx = CallCtx::new(caps);
    let (out, _) = server.tools().call("export", &ctx, call);
    match out {
        Ok(out) => println!("{}", serde_json::to_string_pretty(&out).expect("JSON")),
        Err(e) => {
            eprintln!("{}", e.to_value());
            std::process::exit(1);
        }
    }
}
