//! Writes the toolkit types a person can place, one module to a line, for
//! `src/toolkit.rs`. As `qt_signals` does, this links `qt_types` on the
//! building machine, so a web build carries the names and not the table.
//!
//! The names are what the palette offers: a type that can be written as an
//! object and that the runtime builds, so neither `Layout` (attached only)
//! nor a singleton nor a type Ljós only stands in for.
//!
//! `offers.txt` is what those types can be given, for the inspector: each
//! class once, with its base, so the text stays small enough for the web.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-env-changed=LJOS_QT_TYPES");
    println!("cargo::rerun-if-changed=build.rs");
    let mut out = String::new();
    let mut offers = String::new();
    let mut seen = HashSet::new();
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
        for name in &names {
            if let Some(ty) = module.type_named(name) {
                let _ = writeln!(offers, "E\t{uri}\t{name}\t{}", ty.class);
                describe(ty, &mut seen, &mut offers);
            }
        }
    }
    let dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets it"));
    fs::write(dir.join("toolkit.txt"), out).expect("OUT_DIR is writable");
    fs::write(dir.join("offers.txt"), offers).expect("OUT_DIR is writable");
}

/// A class and what it inherits: `C class base`, then `P name type values`
/// for each thing a person can set.
fn describe(ty: &'static qt_types::Type, seen: &mut HashSet<&'static str>, out: &mut String) {
    if !seen.insert(ty.class) {
        return;
    }
    let base = ty.prototype();
    let _ = writeln!(out, "C\t{}\t{}", ty.class, base.map_or("", |b| b.class));
    let owners = std::iter::once(ty).chain(ty.member_extension());
    for owner in owners {
        for p in &owner.properties {
            offer(ty, p, out);
        }
    }
    if let Some(base) = base {
        describe(base, seen, out);
    }
}

/// `p` of `ty` as what a person can set, or the parts of it where it is a
/// value made of parts (`font.pixelSize`) or a group (`border.color`). Anchors
/// have their own pad, and objects and lists are no value to type.
fn offer(ty: &'static qt_types::Type, p: &'static qt_types::Property, out: &mut String) {
    if p.is_list || p.is_component || p.is_script || p.name == "anchors" {
        return;
    }
    if let Some(value) = p.value_type().filter(|v| !v.properties.is_empty()) {
        if value.is_value || (p.is_readonly && p.is_pointer) {
            for part in value
                .properties
                .iter()
                .filter(|q| !q.is_readonly && !q.is_list)
            {
                line(
                    out,
                    &format!("{}.{}", p.name, part.name),
                    part.type_name,
                    &part_values(value, part),
                );
            }
        }
        return;
    }
    if !p.is_readonly && !p.is_pointer {
        line(out, p.name, p.type_name, &enum_values(ty, p.type_name));
    }
}

fn line(out: &mut String, name: &str, type_name: &str, values: &[String]) {
    let short = type_name.rsplit(['.', ':']).next().unwrap_or(type_name);
    let _ = writeln!(out, "P\t{name}\t{short}\t{}", values.join(" "));
}

/// A part of a value holds a plain number where its values are named on the
/// value's namespace: `font.weight` takes `Font.Bold`, of the enum `FontWeight`.
fn part_values(value: &'static qt_types::Type, part: &qt_types::Property) -> Vec<String> {
    let Some(ty) = qt_types::module(value.module).and_then(|m| m.type_named(&capital(value.name)))
    else {
        return Vec::new();
    };
    let ends = capital(part.name);
    ty.enums
        .iter()
        .find(|e| e.name.ends_with(&ends))
        .map(|e| keys(ty, e))
        .unwrap_or_default()
}

fn capital(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map_or(String::new(), |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

fn enum_values(ty: &'static qt_types::Type, type_name: &str) -> Vec<String> {
    let mut at = Some(ty);
    while let Some(c) = at {
        let named = c
            .enums
            .iter()
            .find(|e| e.name == type_name || e.alias == Some(type_name));
        if let Some(e) = named {
            return keys(c, e);
        }
        at = c.prototype();
    }
    Vec::new()
}

fn keys(owner: &qt_types::Type, e: &qt_types::Enum) -> Vec<String> {
    e.keys
        .iter()
        .map(|(key, _)| format!("{}.{key}", owner.name))
        .collect()
}
