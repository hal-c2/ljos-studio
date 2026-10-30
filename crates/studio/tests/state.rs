//! Studio's state directory outlives no run of Studio: without a runtime
//! directory it lives in persistent storage, where nothing else clears it.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn leftovers(state: &Path) -> Vec<String> {
    std::fs::read_dir(state)
        .map(|dirs| {
            dirs.map(|d| d.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("ljos-studio-"))
                .collect()
        })
        .unwrap_or_default()
}

/// Runs `studio --mcp` on `project` with state kept below `state`, and ends it
/// as a client does, by closing its input.
fn run_and_stop(project: &Path, state: &Path) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_studio"))
        .args(["--mcp", "--ljos", "/nonexistent/ljos"])
        .arg(project)
        .env_remove("XDG_RUNTIME_DIR")
        .env("XDG_STATE_HOME", state)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("the studio program runs");
    let mut stdin = child.stdin.take().expect("piped");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-03-26","capabilities":{{}},"clientInfo":{{"name":"state-test","version":"0"}}}}}}"#
    )
    .unwrap();
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success(), "{status}");
}

#[test]
fn a_run_that_stops_leaves_no_state_directory_behind() {
    let project = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("Main.qml"), "Item {}\n").unwrap();
    run_and_stop(project.path(), state.path());
    run_and_stop(project.path(), state.path());
    assert_eq!(leftovers(state.path()), Vec::<String>::new());
}
