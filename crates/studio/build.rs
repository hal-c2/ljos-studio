//! Compiles the shell Studio ships (`shell/`) against the native types
//! `crates/studio_blocks/ljostypes.json` describes into `$OUT_DIR/shell.pack`,
//! and its bindings and handlers to Rust in `$OUT_DIR/shell.rs`. Its test
//! (`studio_blocks/tests/types.rs`) keeps the type file current.

fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it"));
    if let Err(e) = ljos::build::compile_dirs_with_types_to_rust(
        &["../../shell".as_ref()],
        "../studio_blocks/ljostypes.json".as_ref(),
        &out.join("shell.pack"),
    ) {
        panic!("{e}");
    }
}
