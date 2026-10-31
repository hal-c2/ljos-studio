//! Writes the toolkit types a person can place, one module to a line, for
//! `src/toolkit.rs`. As `qt_signals` does, this links `qt_types` on the
//! building machine, so a web build carries the names and not the table.
//!
//! The names are what the palette offers: a type that can be written as an
//! object and that the runtime builds, so neither `Layout` (attached only)
//! nor a singleton nor a type Ljós only stands in for.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-env-changed=LJOS_QT_TYPES");
    println!("cargo::rerun-if-changed=build.rs");
    let mut out = String::new();
    for uri in qt_types::modules() {
        let Some(module) = qt_types::module(uri) else {
            continue;
        };
        let mut names = qt_types::native_types(uri).unwrap_or_default();
        names.extend(qt_types::written_types(uri).unwrap_or_default());
        names.sort_unstable();
        names.dedup();
        names.retain(|name| {
            module.type_named(name).is_some_and(|ty| {
                ty.is_creatable
                    && ty.is_built
                    && !ty.is_singleton
                    && !ty.is_value
                    && !ty.is_unlisted
            })
        });
        if !names.is_empty() {
            let _ = writeln!(out, "{uri} {}", names.join(" "));
        }
    }
    let dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets it"));
    fs::write(dir.join("toolkit.txt"), out).expect("OUT_DIR is writable");
}
