//! Calls `export` on a project as an agent does and prints the answer, for
//! tasks that need a bundle without an agent (`mise run export:web:check`).
//!
//! `cargo run -p studio_mcp --example export -- DIR ENTRY [native | web [--build]]`
//!
//! `--build` builds the site as Studio's window does, with the ljos-web and
//! `qmlc` that `LJOS_WEB` and `QMLC` name, or that Studio finds.

use ljos_mcp_core::{CallCtx, Cap, CapSet, McpSurface};
use serde_json::json;
use studio_mcp::{Studio, StudioServer};

const USAGE: &str = "usage: export DIR ENTRY [native]\n       export DIR ENTRY web [--build]";

/// The target and whether to build it, from what follows DIR and ENTRY:
/// anything else is refused, so a word misspelt is not a native export.
fn parse(rest: &[String]) -> Result<(String, bool), String> {
    let mut target = None;
    let mut build = false;
    for arg in rest {
        match arg.as_str() {
            "--build" if !build => build = true,
            "native" | "web" if target.is_none() => target = Some(arg.clone()),
            _ => return Err(format!("`{arg}` is not an argument here")),
        }
    }
    let target = target.unwrap_or_else(|| "native".to_owned());
    if build && target != "web" {
        return Err("only a web export is built: give `web`".to_owned());
    }
    Ok((target, build))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect(USAGE);
    let entry = args.next().expect(USAGE);
    let rest: Vec<String> = args.collect();
    let (target, build) = parse(&rest).unwrap_or_else(|e| {
        eprintln!("{e}\n{USAGE}");
        std::process::exit(2);
    });
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

#[cfg(test)]
mod tests {
    use super::parse;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|&a| a.to_owned()).collect()
    }

    #[test]
    fn a_target_and_build_read_as_given() {
        assert_eq!(parse(&args(&[])), Ok(("native".to_owned(), false)));
        assert_eq!(parse(&args(&["native"])), Ok(("native".to_owned(), false)));
        assert_eq!(parse(&args(&["web"])), Ok(("web".to_owned(), false)));
        assert_eq!(
            parse(&args(&["web", "--build"])),
            Ok(("web".to_owned(), true))
        );
        assert_eq!(
            parse(&args(&["--build", "web"])),
            Ok(("web".to_owned(), true))
        );
    }

    // A flag misspelt or a word too many is not silently a native export.
    #[test]
    fn what_it_does_not_know_is_refused() {
        for given in [
            &["--bulid"][..],
            &["web", "--bulid"],
            &["--build"],
            &["native", "--build"],
            &["web", "native"],
            &["web", "--build", "--build"],
            &["phone"],
        ] {
            assert!(parse(&args(given)).is_err(), "{given:?} was read");
        }
    }
}
