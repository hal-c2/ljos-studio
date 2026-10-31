//! `export` (MCP.md): a bundle of the project that runs without Studio.
//!
//! What Ljós runs today without building anything is a project directory
//! and its entry document under `ljos run`; its production bundles (Ljós
//! ARCHITECTURE.md 12.2: hashed manifest, signature, A/B slots) and AOT
//! binaries are not something Studio can make without building an app
//! crate. So a native export is that directory: the project's own files
//! as the shipped app needs them, and a manifest saying what is in it and
//! how to start it. A web site needs ljos-web to build one from QML on its
//! own, which it does not yet; that target is refused, saying so.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use ljos_blocks::read_qmldir;
use ljos_mcp_core::{Cap, ErrorCode, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, typed};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Studio, blocks, lock, preview};

/// The manifest's name, at the bundle's root.
pub const MANIFEST: &str = "ljos-bundle.json";
const FORMAT: &str = "ljos-bundle";
const VERSION: u32 = 1;
/// Bundles are written below this directory of the project, which is not
/// part of its documents (the store does not list it), so a bundle never
/// becomes part of the design nor of the next bundle.
const BUILD: &str = "build";
/// The project's mock modules (`preview_open`'s `mocked` state).
const MOCKS: &str = "mocks";

/// How many files a bundle may hold through links it cannot keep as links.
/// Links that fan out and meet again copy a directory once per way down,
/// which doubles with every level.
const LINKED_FILES: usize = 10_000;

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    /// A directory `ljos run` starts.
    #[default]
    Native,
    /// A static site (not built yet: refused with the reason).
    Web,
}

impl Target {
    fn word(self) -> &'static str {
        match self {
            Target::Native => "native",
            Target::Web => "web",
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExportArgs {
    /// The document the app starts with, usually its shell (`Shell.qml`).
    entry: String,
    /// `native` (default) or `web`.
    #[serde(default)]
    target: Target,
    /// Where, relative to the project and below its `build/`; default
    /// `build/<target>`. A previous bundle there is replaced.
    out: Option<String>,
    /// Say what the bundle would hold; write nothing.
    #[serde(default)]
    dry_run: bool,
}

/// What a bundle holds; written beside it as `ljos-bundle.json`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub target: Target,
    pub entry: String,
    /// The command that starts it, run in the bundle's directory.
    pub run: Vec<String>,
    /// The theme file the app reads, beside its entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The project's own blocks, as `Module.Name`, and their files.
    pub blocks: Vec<BlockFile>,
    /// Every file, by path relative to the bundle.
    pub files: Vec<FileEntry>,
    /// Links to directories the bundle holds, so what the app reads
    /// through a link's name is there too: written as links where the
    /// system makes them, and where it does not as a copy of the
    /// directory under the link's name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LinkEntry>,
    /// What of the project is not in the bundle, and why.
    pub left_out: Vec<LeftOut>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LinkEntry {
    /// Where the link is, relative to the bundle.
    pub path: String,
    /// The directory it leads to, relative to the bundle.
    pub to: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BlockFile {
    pub block: String,
    pub file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LeftOut {
    pub path: String,
    pub why: String,
}

#[derive(Serialize, JsonSchema)]
struct ExportOut {
    /// The bundle's directory, relative to the project.
    out: String,
    /// How many files the bundle holds and their size, which stay true when
    /// a long manifest is cut short in this answer (the one written is whole),
    /// copies of links it could not hold as links included.
    files: usize,
    bytes: u64,
    manifest: Manifest,
    /// False for a dry run.
    written: bool,
    ms: f64,
}

fn refused(code: ErrorCode, kind: &str, message: impl Into<String>) -> SurfaceError {
    SurfaceError::new(code, message).with_details(json!({ "kind": kind }))
}

fn io(path: &Path, e: &std::io::Error) -> SurfaceError {
    SurfaceError::new(ErrorCode::Internal, format!("{}: {e}", path.display()))
}

/// Why a directory or file of the project is not in its bundle; `None`
/// for one that is. `top` says whether it is directly in the project.
fn left_out_why(name: &str, is_dir: bool, top: bool) -> Option<&'static str> {
    match name {
        ".studio" if top => Some("Studio's sidecar"),
        MOCKS if top && is_dir => Some("mock modules stand in for blocks in previews only"),
        // The directories `ljos run` does not read a project's files from.
        BUILD | "target" | "node_modules" if is_dir => Some("build output and tooling"),
        _ if is_dir && (name.starts_with("build-") || name.starts_with("cmake-build")) => {
            Some("build output and tooling")
        }
        _ if name.starts_with('.') => Some("hidden: not part of the app"),
        // Half a file Studio was writing when it stopped.
        _ if name.ends_with(".studio-tmp") => Some("an unfinished write"),
        _ => None,
    }
}

/// Why a bundle leaves out where a link in the project leads: any part of
/// the way from `root` to `real` that the walk would not go down itself.
fn target_left_out_why(root: &Path, real: &Path, is_dir: bool) -> Option<&'static str> {
    let parts: Vec<_> = real.strip_prefix(root).ok()?.components().collect();
    parts.iter().enumerate().find_map(|(at, part)| {
        let last = at + 1 == parts.len();
        left_out_why(
            &part.as_os_str().to_string_lossy(),
            !last || is_dir,
            at == 0,
        )
    })
}

/// The project's files that go in a bundle, sorted, and what is left out.
/// `root` is canonical.
///
/// A link to a file in the project is copied as the file. A link to a
/// directory is not walked: every directory of the project is walked under
/// its own name or left out by the rules above, so the bundle holds where
/// the link leads under that name, and the link goes in as a link to it
/// (`Manifest::links`), which keeps an app that reads the directory
/// through the link's name working without a second copy. A link to a
/// directory it is in stays out (the bundle would hold itself), as does
/// one to what the bundle leaves out, saying why, and one out of the
/// project, so a link a checked-out project brings cannot copy somebody's
/// directory into the bundle.
fn walk(root: &Path) -> Result<Walked, SurfaceError> {
    let mut files = Vec::new();
    let mut left_out = Vec::new();
    // Each real directory walked, with where it is in the bundle.
    let mut walked = HashMap::from([(root.to_owned(), String::new())]);
    let mut stack = vec![(root.to_owned(), String::new())];
    // Links to directories, answered once every directory has been walked,
    // since one may lead to a directory the walk has yet to reach.
    let mut links: Vec<(String, PathBuf)> = Vec::new();
    while let Some((dir, prefix)) = stack.pop() {
        let mut entries = fs::read_dir(&dir)
            .map_err(|e| io(&dir, &e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| io(&dir, &e))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = format!("{prefix}{name}");
            let full = entry.path();
            let link = fs::symlink_metadata(&full)
                .map_err(|e| io(&full, &e))?
                .file_type()
                .is_symlink();
            let real = if link {
                match full.canonicalize() {
                    Ok(real) if real.starts_with(root) => real,
                    _ => {
                        left_out.push(LeftOut {
                            path,
                            why: "a link out of the project".to_owned(),
                        });
                        continue;
                    }
                }
            } else {
                dir.join(&name)
            };
            let meta = fs::metadata(&full).map_err(|e| io(&full, &e))?;
            let own = left_out_why(&name, meta.is_dir(), prefix.is_empty());
            // A link's name says nothing: `shim -> mocks` is still mocks.
            let target = if link {
                target_left_out_why(root, &real, meta.is_dir())
            } else {
                None
            };
            // A pipe's reader waits for a writer, a device's has no end:
            // the export would hold Studio until somebody wrote one.
            let special = (!meta.is_dir() && !meta.is_file())
                .then_some("not a regular file: a pipe, socket or device");
            if let Some(why) = own.or(target).or(special) {
                left_out.push(LeftOut {
                    path,
                    why: why.to_owned(),
                });
            } else if meta.is_dir() && link {
                links.push((path, real));
            } else if meta.is_dir() {
                walked.insert(real, path.clone());
                stack.push((full, format!("{path}/")));
            } else {
                files.push(FileEntry {
                    path,
                    bytes: meta.len(),
                });
            }
        }
    }
    let mut kept = Vec::new();
    for (path, real) in links {
        left_out.push(match walked.get(&real) {
            Some(at) if !(at.is_empty() || path.starts_with(&format!("{at}/"))) => {
                kept.push(LinkEntry {
                    path,
                    to: at.clone(),
                });
                continue;
            }
            Some(at) => already(path, at),
            // Never met while `target_left_out_why` leaves out every
            // directory the walk does not go down; said rather than
            // assumed, so the two cannot drift into copying it.
            None => LeftOut {
                path,
                why: "a link to a directory the bundle does not hold".to_owned(),
            },
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    kept.sort_by(|a, b| a.path.cmp(&b.path));
    left_out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((files, kept, left_out))
}

/// The files, the links to directories and what is left out, as `walk`
/// finds them.
type Walked = (Vec<FileEntry>, Vec<LinkEntry>, Vec<LeftOut>);

/// Why the link at `path` to the directory the bundle has at `at`, one it
/// is in, is left out.
fn already(path: String, at: &str) -> LeftOut {
    let why = if at.is_empty() {
        "a link to the project, which the bundle holds already".to_owned()
    } else {
        format!("a link to `{at}`, which it is in and the bundle holds already")
    };
    LeftOut { path, why }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn module_of(qmldir: &str) -> Option<String> {
    qmldir.lines().find_map(|l| {
        let mut words = l.split_whitespace();
        (words.next() == Some("module"))
            .then(|| words.next().map(str::to_owned))
            .flatten()
    })
}

/// Checks what the bundle would hold and says what it is.
fn plan(
    root: &Path,
    studio: &Studio,
    entry: &str,
    target: Target,
) -> Result<Manifest, SurfaceError> {
    if target == Target::Web {
        return Err(refused(
            ErrorCode::Unsupported,
            "no_web_build",
            "a site for the web is not built yet: ljos-web compiles QML to modules that need its \
             runtime package and a bundler, and builds no site on its own; export `native` and \
             run it with `ljos run`",
        ));
    }
    let store = studio.session.store();
    if !entry.ends_with(".qml") || !store.exists(entry) {
        return Err(SurfaceError::not_found(format!(
            "no document `{entry}` to start the app with"
        )));
    }
    let (files, links, left_out) = walk(root)?;
    // The store has every file of the project; the bundle only what the
    // walk keeps, and a manifest that starts a file it does not hold
    // would not start.
    if !files.iter().any(|f| f.path == entry) {
        let why = left_out
            .iter()
            .find(|l| entry == l.path || entry.starts_with(&format!("{}/", l.path)))
            .map_or("not a file of the project", |l| l.why.as_str());
        return Err(SurfaceError::new(
            ErrorCode::NotActionable,
            format!("`{entry}` is left out of the bundle ({why}); start the app with a document it holds"),
        )
        .with_details(json!({ "kind": "entry_left_out", "why": why })));
    }
    let read = |path: &str| {
        let full = root.join(path);
        fs::read_to_string(&full).map_err(|e| io(&full, &e))
    };

    // A document that does not parse would ship an app that does not start
    // or a screen of it that does not show.
    for path in files
        .iter()
        .map(|f| &f.path)
        .filter(|p| p.ends_with(".qml"))
    {
        let text = read(path)?;
        if studio_core::outline::outline(&text).is_none() {
            return Err(SurfaceError::new(
                ErrorCode::Syntax,
                format!("`{path}` does not parse; mend it before exporting"),
            )
            .with_details(json!({ "kind": "broken", "doc": path })));
        }
    }

    // Types and blocks in Rust are registered by the app's own program;
    // `ljos run` has none of them, so the bundle would not start.
    let native: Vec<&str> = files
        .iter()
        .map(|f| f.path.as_str())
        .filter(|p| file_name(p) == "ljostypes.json")
        .collect();
    if !native.is_empty() {
        return Err(SurfaceError::new(
            ErrorCode::Unsupported,
            "the project has types written in Rust, which only the app's own program registers; \
             `ljos run` cannot start it",
        )
        .with_details(json!({ "kind": "native_types", "files": native })));
    }

    let mut live = Vec::new();
    let mut blocks_out = Vec::new();
    for path in files
        .iter()
        .map(|f| &f.path)
        .filter(|p| file_name(p) == "qmldir")
    {
        let text = read(path)?;
        let Some(module) = module_of(&text) else {
            continue;
        };
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        for (_, decl) in read_qmldir(&text).blocks {
            if let Some(file) = blocks::join(dir, &decl.file) {
                blocks_out.push(BlockFile {
                    block: format!("{module}.{}", decl.name),
                    file,
                });
            }
        }
        live.push(module);
    }
    blocks_out.sort_by(|a, b| a.block.cmp(&b.block));
    let mocked_only: Vec<String> = preview::mocks(store)
        .into_iter()
        .map(|a| a.module)
        .filter(|m| !live.contains(m))
        .collect();
    if !mocked_only.is_empty() {
        return Err(SurfaceError::new(
            ErrorCode::NotActionable,
            format!(
                "only a mock provides {}: the app has nothing live to ship in its place",
                mocked_only.join(", ")
            ),
        )
        .with_details(json!({ "kind": "mocked_only", "modules": mocked_only })));
    }

    let beside = entry.rsplit_once('/').map_or("", |(dir, _)| dir);
    let theme_path = if beside.is_empty() {
        "theme.json".to_owned()
    } else {
        format!("{beside}/theme.json")
    };
    let theme = files
        .iter()
        .any(|f| f.path == theme_path)
        .then_some(theme_path);
    let manifest = Manifest {
        format: FORMAT.to_owned(),
        version: VERSION,
        target,
        entry: entry.to_owned(),
        run: ["ljos", "run", entry, "--project", "."]
            .map(str::to_owned)
            .to_vec(),
        theme,
        blocks: blocks_out,
        files,
        links,
        left_out,
    };
    Ok(manifest)
}

/// `out` as a place below the project's `build/`, by its names and on
/// disk: `root` is canonical, and a link on the way (a `build` a
/// checked-out project brings, say) would send the bundle's write and the
/// removal of the last one somewhere else.
fn out_dir(root: &Path, out: Option<&str>, target: Target) -> Result<String, SurfaceError> {
    let out = out.map_or_else(|| format!("{BUILD}/{}", target.word()), str::to_owned);
    // By its components, for the checks and the write alike: the system
    // resolves a last `.` through a link that `build/native/.` as written
    // would not show as one, and removing it empties what the link is to.
    let path: PathBuf = Path::new(&out).components().collect();
    let plain = path.components().all(|c| matches!(c, Component::Normal(_)));
    let mut parts = path.components();
    let below_build =
        parts.next().is_some_and(|c| c.as_os_str() == BUILD) && parts.next().is_some();
    if !plain || !below_build {
        return Err(SurfaceError::new(
            ErrorCode::PathDenied,
            format!("`{out}` is not below the project's `{BUILD}/`, where bundles are written"),
        ));
    }
    let full = root.join(&path);
    if leads_elsewhere(root, &full) {
        return Err(SurfaceError::new(
            ErrorCode::PathDenied,
            format!(
                "`{out}` goes through a link; bundles are written to the project's own `{BUILD}/`"
            ),
        ));
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Whether a part of `full` below `root` that exists is a link. One out of
/// the project is somebody else's directory; even one that stays in it
/// would put the bundle where the walk reads the design from.
fn leads_elsewhere(root: &Path, full: &Path) -> bool {
    let mut at = full;
    while at != root {
        if fs::symlink_metadata(at).is_ok_and(|m| m.file_type().is_symlink()) {
            return true;
        }
        match at.parent() {
            Some(parent) => at = parent,
            None => return true,
        }
    }
    false
}

/// Whether `dir` holds a bundle's own manifest: a file, not a link to
/// one, that says it is a manifest. A file only named like one is
/// somebody's, and the directory with it.
fn holds_bundle(dir: &Path) -> bool {
    let at = dir.join(MANIFEST);
    fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_file())
        && fs::read_to_string(&at)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|v| v.get("format").and_then(|f| f.as_str()) == Some(FORMAT))
}

/// Refuses an `out` that leads elsewhere now, or that holds something other
/// than a bundle or nothing. Asked again just before `out` is removed, so
/// a directory swapped in while the files were copied is not emptied.
fn may_replace(root: &Path, out: &Path) -> Result<(), SurfaceError> {
    if leads_elsewhere(root, out) {
        return Err(SurfaceError::new(
            ErrorCode::PathDenied,
            format!(
                "`{}` goes through a link now; bundles are written to the project's own `{BUILD}/`",
                out.display()
            ),
        ));
    }
    let Ok(found) = fs::symlink_metadata(out) else {
        return Ok(());
    };
    let empty = || fs::read_dir(out).is_ok_and(|mut d| d.next().is_none());
    if !found.is_dir() || !(holds_bundle(out) || empty()) {
        return Err(refused(
            ErrorCode::InvalidArgument,
            "exists",
            format!(
                "`{}` holds something other than a bundle; choose another `out`",
                out.display()
            ),
        ));
    }
    Ok(())
}

/// The files and bytes a bundle holds besides its manifest's files: what
/// copies of links put under the links' names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Copied {
    files: usize,
    bytes: u64,
}

/// Copies the files into a place beside `out` and swaps it in, so a
/// bundle is never half one export and half another.
fn write(root: &Path, out: &Path, manifest: &Manifest) -> Result<Copied, SurfaceError> {
    write_with(root, out, manifest, link_dir, || {})
}

/// A link at `at` to the directory `to`, relative to where `at` is.
fn link_dir(to: &Path, at: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(to, at)
    }
    // Refused to a user without the privilege to make links.
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(to, at)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (to, at);
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// [`write`]; `link` makes a link to a directory, which a test refuses, and
/// `copied` runs once the files are copied and before `out` is replaced,
/// where a test swaps a directory.
fn write_with(
    root: &Path,
    out: &Path,
    manifest: &Manifest,
    link: impl Fn(&Path, &Path) -> std::io::Result<()>,
    copied: impl FnOnce(),
) -> Result<Copied, SurfaceError> {
    may_replace(root, out)?;
    let parent = out.parent().expect("below build/");
    let name = out.file_name().expect("a name").to_string_lossy();
    let staging: PathBuf = parent.join(format!(".{name}.partial"));
    // Not followed: a link left here is let go of, not emptied.
    if fs::symlink_metadata(&staging).is_ok() {
        fs::remove_dir_all(&staging).map_err(|e| io(&staging, &e))?;
    }
    fs::create_dir_all(&staging).map_err(|e| io(&staging, &e))?;
    // The links first: where the system makes none, what their copies
    // would write is counted before any file is, so a copy too large is
    // refused with nothing written.
    let mut unmade = Vec::new();
    for entry in &manifest.links {
        if !place_link(&staging, &entry.path, &entry.to, &link)? {
            unmade.push(entry);
        }
    }
    let through = match through_links(&manifest.files, &manifest.links, &unmade) {
        Ok(through) => through,
        Err(refusal) => {
            fs::remove_dir_all(&staging).map_err(|e| io(&staging, &e))?;
            return Err(refusal);
        }
    };
    for path in manifest.files.iter().map(|f| &f.path) {
        copy_file(root, path, &staging.join(path))?;
    }
    for (path, file) in &through {
        copy_file(root, &file.path, &staging.join(path))?;
    }
    let mut text = serde_json::to_string_pretty(manifest).expect("a manifest is JSON");
    text.push('\n');
    let at = staging.join(MANIFEST);
    fs::write(&at, text).map_err(|e| io(&at, &e))?;
    copied();
    // Path by path, so what is checked is what is there now: what is left
    // between this and the removal is a window no path can close (see the
    // DECISION in DESIGN.md's S6 risks).
    may_replace(root, out)?;
    if leads_elsewhere(root, &staging) {
        return Err(SurfaceError::new(
            ErrorCode::PathDenied,
            format!("`{}` goes through a link now", staging.display()),
        ));
    }
    if fs::symlink_metadata(out).is_ok() {
        fs::remove_dir_all(out).map_err(|e| io(out, &e))?;
    }
    fs::rename(&staging, out).map_err(|e| io(out, &e))?;
    Ok(Copied {
        files: through.len(),
        bytes: through.iter().map(|(_, f)| f.bytes).sum(),
    })
}

/// Copies the project's `path` to `to`, as the walk found it: a file
/// swapped since then for a link out of the project, or to what the bundle
/// leaves out (`.studio/`, the mocks, build output), or for a pipe, is not
/// read.
fn copy_file(root: &Path, path: &str, to: &Path) -> Result<(), SurfaceError> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| io(dir, &e))?;
    }
    let from = root.join(path);
    let still = from.canonicalize().is_ok_and(|real| {
        real.starts_with(root) && target_left_out_why(root, &real, false).is_none()
    }) && fs::metadata(&from).is_ok_and(|m| m.is_file());
    if !still {
        return Err(refused(
            ErrorCode::NotActionable,
            "changed",
            format!("`{path}` changed while the bundle was written; export again"),
        ));
    }
    fs::copy(&from, to).map_err(|e| io(&from, &e))?;
    Ok(())
}

/// Puts the link at `path` to the bundle's directory `to` in `staging`;
/// false where `link` makes none, and the directory is to be copied there.
fn place_link(
    staging: &Path,
    path: &str,
    to: &str,
    link: &impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<bool, SurfaceError> {
    let at = staging.join(path);
    if let Some(dir) = at.parent() {
        fs::create_dir_all(dir).map_err(|e| io(dir, &e))?;
    }
    // A directory without files has no place in the bundle yet.
    let target = staging.join(to);
    fs::create_dir_all(&target).map_err(|e| io(&target, &e))?;
    let relative: PathBuf = std::iter::repeat_n("..", path.matches('/').count())
        .collect::<PathBuf>()
        .join(to);
    if link(&relative, &at).is_ok() {
        return Ok(true);
    }
    fs::create_dir_all(&at).map_err(|e| io(&at, &e))?;
    Ok(false)
}

/// Where a copy of each of `copied` puts the files of the directory it
/// leads to, under its name, with the links in that directory copied the
/// same way, and which of `files` each one is. Each `(path, to)` is copied
/// once, and a link back into a directory being copied around it is not
/// followed again. Past [`LINKED_FILES`] it is refused before anything is
/// copied.
fn through_links<'a>(
    files: &'a [FileEntry],
    links: &[LinkEntry],
    copied: &[&LinkEntry],
) -> Result<Vec<(String, &'a FileEntry)>, SurfaceError> {
    struct Copy<'a, 'l> {
        files: &'a [FileEntry],
        links: &'l [LinkEntry],
        copying: Vec<&'l str>,
        done: BTreeSet<(String, &'l str)>,
        out: Vec<(String, &'a FileEntry)>,
    }
    impl<'l> Copy<'_, 'l> {
        fn link(&mut self, path: String, to: &'l str) -> Result<(), SurfaceError> {
            if self.copying.contains(&to) || !self.done.insert((path.clone(), to)) {
                return Ok(());
            }
            self.copying.push(to);
            let below = format!("{to}/");
            for file in self.files.iter().filter(|f| f.path.starts_with(&below)) {
                if self.out.len() == LINKED_FILES {
                    return Err(refused(
                        ErrorCode::Unsupported,
                        "links_multiply",
                        format!(
                            "the project's links lead into the same directories along so many \
                             ways that copying them, where the bundle cannot hold them as links, \
                             would write more than {LINKED_FILES} files; link each directory once"
                        ),
                    ));
                }
                self.out
                    .push((format!("{path}/{}", &file.path[below.len()..]), file));
            }
            let links = self.links;
            for inner in links.iter().filter(|l| l.path.starts_with(&below)) {
                self.link(format!("{path}/{}", &inner.path[below.len()..]), &inner.to)?;
            }
            self.copying.pop();
            Ok(())
        }
    }
    let mut copy = Copy {
        files,
        links,
        copying: Vec::new(),
        done: BTreeSet::new(),
        out: Vec::new(),
    };
    for link in copied {
        copy.link(link.path.clone(), &link.to)?;
    }
    Ok(copy.out)
}

fn export(studio: &Mutex<Studio>, a: ExportArgs) -> Result<ExportOut, SurfaceError> {
    let studio = lock(studio);
    let started = studio.timer.now();
    let Some(root) = studio.dir() else {
        return Err(refused(
            ErrorCode::NotActionable,
            "no_directory",
            "this project is not in a directory: there are no files to bundle",
        ));
    };
    let root = root.canonicalize().map_err(|e| io(&root, &e))?;
    let manifest = plan(&root, &studio, &a.entry, a.target)?;
    let out = out_dir(&root, a.out.as_deref(), a.target)?;
    let copied = if a.dry_run {
        Copied::default()
    } else {
        write(&root, &root.join(&out), &manifest)?
    };
    Ok(ExportOut {
        out,
        files: manifest.files.len() + copied.files,
        bytes: manifest.files.iter().map(|f| f.bytes).sum::<u64>() + copied.bytes,
        manifest,
        written: !a.dry_run,
        ms: studio_core::preview::ms(studio.timer.now().saturating_sub(started)),
    })
}

pub(crate) fn add(registry: ToolRegistry, studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let studio = studio.clone();
    registry.with(
        ToolDef::new::<ExportArgs>(
            "export",
            "Write a bundle of the project that runs without Studio: its QML, theme, assets and \
             own blocks, never mocks or `.studio/`, with a manifest (`ljos-bundle.json`) saying \
             what it holds and how to start it (`ljos run`). `dry_run` writes nothing.",
        )
        .output::<ExportOut>()
        .requires(Cap::Fs)
        .annotated(ToolAnnotations {
            read_only: false,
            destructive: true,
            idempotent: true,
            open_world: false,
        }),
        typed(move |_, a: ExportArgs| export(&studio, a)),
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    fn manifest(files: &[&str]) -> Manifest {
        Manifest {
            format: FORMAT.to_owned(),
            version: VERSION,
            target: Target::Native,
            entry: files[0].to_owned(),
            run: Vec::new(),
            theme: None,
            blocks: Vec::new(),
            files: files
                .iter()
                .map(|p| FileEntry {
                    path: (*p).to_owned(),
                    bytes: 0,
                })
                .collect(),
            links: Vec::new(),
            left_out: Vec::new(),
        }
    }

    /// What `plan` would bundle of `root`, entry aside.
    fn walked(root: &Path) -> Manifest {
        let (files, links, left_out) = walk(root).unwrap();
        Manifest {
            files,
            links,
            left_out,
            ..manifest(&[""])
        }
    }

    fn refuse(_: &Path, _: &Path) -> std::io::Result<()> {
        Err(std::io::ErrorKind::PermissionDenied.into())
    }

    fn file(root: &Path, path: &str) {
        let at = root.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, path).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_build_directory_swapped_for_a_link_while_copying_is_not_emptied() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        fs::write(root.join("Shell.qml"), "Item {}\n").unwrap();
        // Somebody's directory that looks like a bundle from its manifest.
        let outside = tempfile::tempdir().unwrap();
        let theirs = outside.path().join("native");
        fs::create_dir(&theirs).unwrap();
        fs::write(theirs.join(MANIFEST), r#"{"format": "ljos-bundle"}"#).unwrap();
        fs::write(theirs.join("precious.txt"), "keep me").unwrap();
        let out = root.join(BUILD).join("native");
        let swapped = write_with(&root, &out, &manifest(&["Shell.qml"]), link_dir, || {
            fs::rename(root.join(BUILD), root.join("build.was")).unwrap();
            symlink(outside.path(), root.join(BUILD)).unwrap();
        });
        let refused = swapped.unwrap_err();
        assert_eq!(refused.code, ErrorCode::PathDenied, "{refused:?}");
        assert_eq!(names(&theirs), [MANIFEST, "precious.txt"]);
    }

    #[test]
    fn a_file_swapped_for_a_link_out_after_the_walk_is_not_copied() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "theirs").unwrap();
        // What the walk saw was a file of the project.
        let checked = manifest(&["Shell.qml"]);
        symlink(outside.path().join("secret.txt"), root.join("Shell.qml")).unwrap();
        let out = root.join(BUILD).join("native");
        let refused = write(&root, &out, &checked).unwrap_err();
        assert_eq!(refused.code, ErrorCode::NotActionable, "{refused:?}");
        assert!(!out.exists());
        assert!(!root.join(BUILD).join(".native.partial/Shell.qml").exists());
    }

    #[test]
    fn a_file_swapped_for_a_link_to_what_is_left_out_after_the_walk_is_not_copied() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        fs::create_dir_all(root.join(".studio")).unwrap();
        fs::write(root.join(".studio/review.json"), "{}").unwrap();
        // What the walk saw was a file of the project.
        let checked = manifest(&["Shell.qml"]);
        symlink(root.join(".studio/review.json"), root.join("Shell.qml")).unwrap();
        let out = root.join(BUILD).join("native");
        let refused = write(&root, &out, &checked).unwrap_err();
        assert_eq!(refused.code, ErrorCode::NotActionable, "{refused:?}");
        assert!(!out.exists());
        assert!(!root.join(BUILD).join(".native.partial/Shell.qml").exists());
    }

    #[test]
    fn a_link_to_a_directory_the_bundle_holds_goes_in_as_a_link_to_it() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        file(&root, "zone/Part.qml");
        fs::create_dir(root.join("ui")).unwrap();
        symlink(root.join("zone"), root.join("ui/alias")).unwrap();
        let bundle = walked(&root);
        assert_eq!(
            bundle.links,
            [LinkEntry {
                path: "ui/alias".to_owned(),
                to: "zone".to_owned()
            }]
        );
        let out = root.join(BUILD).join("native");
        write(&root, &out, &bundle).unwrap();
        assert_eq!(
            fs::read_link(out.join("ui/alias")).unwrap(),
            Path::new("../zone")
        );
        assert_eq!(
            fs::read_to_string(out.join("ui/alias/Part.qml")).unwrap(),
            "zone/Part.qml"
        );
    }

    #[test]
    fn where_no_link_can_be_made_the_directory_is_copied_under_its_name() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        file(&root, "zone/Part.qml");
        file(&root, "deep/More.qml");
        symlink(root.join("deep"), root.join("zone/inner")).unwrap();
        symlink(root.join("zone"), root.join("alias")).unwrap();
        let out = root.join(BUILD).join("native");
        write_with(&root, &out, &walked(&root), refuse, || {}).unwrap();
        for path in [
            "alias/Part.qml",
            "alias/inner/More.qml",
            "zone/inner/More.qml",
        ] {
            let at = out.join(path);
            let meta = fs::symlink_metadata(&at).unwrap();
            assert!(meta.is_file(), "{path}");
        }
        assert!(
            !fs::symlink_metadata(out.join("alias"))
                .unwrap()
                .is_symlink()
        );
    }

    #[test]
    fn links_that_lead_into_each_other_copy_each_directory_once_along_the_way() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        file(&root, "top/Top.qml");
        file(&root, "zone/Part.qml");
        symlink(root.join("zone"), root.join("top/a")).unwrap();
        symlink(root.join("top"), root.join("zone/b")).unwrap();
        let out = root.join(BUILD).join("native");
        write_with(&root, &out, &walked(&root), refuse, || {}).unwrap();
        assert!(out.join("top/a/b/Top.qml").is_file());
        assert!(!out.join("top/a/b/a").exists());
        assert!(out.join("zone/b/a/Part.qml").is_file());
        assert!(!out.join("zone/b/a/b").exists());
    }

    /// `levels` directories under `levels/`, each with a `level.txt` and,
    /// but the last, links `left` and `right` to the next.
    fn diamond(root: &Path, levels: usize) {
        for level in 0..levels {
            let dir = root.join("levels").join(level.to_string());
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("level.txt"), format!("{level}\n")).unwrap();
            if level + 1 < levels {
                for side in ["left", "right"] {
                    let next = root.join("levels").join((level + 1).to_string());
                    symlink(next, dir.join(side)).unwrap();
                }
            }
        }
    }

    #[test]
    fn a_diamond_of_links_too_large_to_copy_is_refused_before_anything_is_written() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        diamond(&root, 24);
        let out = root.join(BUILD).join("native");
        let started = std::time::Instant::now();
        let refusal = write_with(&root, &out, &walked(&root), refuse, || {}).unwrap_err();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(refusal.details.as_ref().unwrap()["kind"], "links_multiply");
        assert!(!out.exists());
        assert!(!root.join(BUILD).join(".native.partial").exists());
    }

    #[test]
    fn a_small_diamond_of_links_is_copied_once_per_way_down_and_counted() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        diamond(&root, 3);
        let out = root.join(BUILD).join("native");
        let copied = write_with(&root, &out, &walked(&root), refuse, || {}).unwrap();
        // Level 1 by two ways, each holding level 2 by two more; level 2 by
        // two ways from level 1 itself: 2 × 3 + 2 files, of two bytes each.
        assert_eq!(
            copied,
            Copied {
                files: 8,
                bytes: 16
            }
        );
        for path in [
            "levels/0/left/level.txt",
            "levels/0/right/right/level.txt",
            "levels/1/left/level.txt",
        ] {
            assert!(out.join(path).is_file(), "{path}");
        }
    }
}
