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
    /// What of the project is not in the bundle, and why.
    pub left_out: Vec<LeftOut>,
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
    /// a long manifest is cut short in this answer (the one written is whole).
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
/// A link is followed as `ljos run` would read through it while it stays
/// in the project; one that leads out of it is left out, so a link a
/// checked-out project brings cannot copy somebody's directory into the
/// bundle, and one back to a directory the walk is inside would never end.
fn walk(root: &Path) -> Result<(Vec<FileEntry>, Vec<LeftOut>), SurfaceError> {
    let mut files = Vec::new();
    let mut left_out = Vec::new();
    // Each directory with the real directories it is below, itself first.
    let mut stack = vec![(root.to_owned(), String::new(), vec![root.to_owned()])];
    while let Some((dir, prefix, within)) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| io(&dir, &e))?;
        for entry in entries {
            let entry = entry.map_err(|e| io(&dir, &e))?;
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
                within[0].join(&name)
            };
            let meta = fs::metadata(&full).map_err(|e| io(&full, &e))?;
            let own = left_out_why(&name, meta.is_dir(), prefix.is_empty());
            // A link's name says nothing: `shim -> mocks` is still mocks.
            let target = if link {
                target_left_out_why(root, &real, meta.is_dir())
            } else {
                None
            };
            if let Some(why) = own.or(target) {
                left_out.push(LeftOut {
                    path,
                    why: why.to_owned(),
                });
            } else if meta.is_dir() && within.contains(&real) {
                left_out.push(LeftOut {
                    path,
                    why: "a link back to a directory it is in".to_owned(),
                });
            } else if meta.is_dir() {
                let mut below = vec![real];
                below.extend(within.iter().cloned());
                stack.push((full, format!("{path}/"), below));
            } else {
                files.push(FileEntry {
                    path,
                    bytes: meta.len(),
                });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    left_out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((files, left_out))
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
    let (files, left_out) = walk(root)?;
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

/// Copies the files into a place beside `out` and swaps it in, so a
/// bundle is never half one export and half another.
fn write(root: &Path, out: &Path, manifest: &Manifest) -> Result<(), SurfaceError> {
    if out.exists() {
        let previous = out.join(MANIFEST).is_file();
        let empty = fs::read_dir(out).is_ok_and(|mut d| d.next().is_none());
        if !previous && !empty {
            return Err(refused(
                ErrorCode::InvalidArgument,
                "exists",
                format!(
                    "`{}` holds something other than a bundle; choose another `out`",
                    out.display()
                ),
            ));
        }
    }
    let parent = out.parent().expect("below build/");
    let name = out.file_name().expect("a name").to_string_lossy();
    let staging: PathBuf = parent.join(format!(".{name}.partial"));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| io(&staging, &e))?;
    }
    fs::create_dir_all(&staging).map_err(|e| io(&staging, &e))?;
    for path in manifest.files.iter().map(|f| &f.path) {
        let to = staging.join(path);
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).map_err(|e| io(dir, &e))?;
        }
        let from = root.join(path);
        fs::copy(&from, &to).map_err(|e| io(&from, &e))?;
    }
    let mut text = serde_json::to_string_pretty(manifest).expect("a manifest is JSON");
    text.push('\n');
    let at = staging.join(MANIFEST);
    fs::write(&at, text).map_err(|e| io(&at, &e))?;
    if out.exists() {
        fs::remove_dir_all(out).map_err(|e| io(out, &e))?;
    }
    fs::rename(&staging, out).map_err(|e| io(out, &e))
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
    if !a.dry_run {
        write(&root, &root.join(&out), &manifest)?;
    }
    Ok(ExportOut {
        out,
        files: manifest.files.len(),
        bytes: manifest.files.iter().map(|f| f.bytes).sum(),
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
