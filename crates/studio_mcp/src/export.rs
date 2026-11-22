//! `export` (MCP.md): a bundle of the project that runs without Studio.
//!
//! What Ljós runs today without building anything is a project directory
//! and its entry document under `ljos run`; its production bundles (Ljós
//! ARCHITECTURE.md 12.2: hashed manifest, signature, A/B slots) and AOT
//! binaries are not something Studio can make without building an app
//! crate. So a native export is that directory: the project's own files
//! as the shipped app needs them, and a manifest saying what is in it and
//! how to start it.
//!
//! A web export is the same walk written for a site builder (DESIGN.md S6,
//! the site contract): the files, and a manifest (`ljos-site.json`) saying
//! where the site starts, where the project's modules are, what it imports
//! from ljos-web and which ljos-web it was checked against. Compiling and
//! bundling are ljos-web's; choosing the files and refusing what has no
//! face on the web are Studio's, so a site builder is never handed a
//! project it can only half build.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, PoisonError},
};

use crate::schema::Answers;
use ljos_blocks::read_qmldir;
use ljos_mcp_core::{
    CallCtx, Cap, ErrorCode, SurfaceError, ToolAnnotations, ToolDef, ToolRegistry, typed,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Studio, blocks, lock, preview};

/// The manifest's name, at the bundle's root.
pub const MANIFEST: &str = "ljos-bundle.json";
const FORMAT: &str = "ljos-bundle";
const VERSION: u32 = 1;
/// A web export's manifest, at the site bundle's root.
pub const SITE_MANIFEST: &str = "ljos-site.json";
const SITE_FORMAT: &str = "ljos-site";
const SITE_VERSION: u32 = 1;
/// The ljos-web commit Studio is checked against; its one place is
/// `web/ljos-web.rev`, which the tasks and CI read too.
pub const LJOS_WEB: &str = include_str!("../../../web/ljos-web.rev").trim_ascii();
/// The modules a page can import from ljos-web at `LJOS_WEB`: its runtime
/// package's exports (`packages/runtime/package.json`), `ljos-web/QtQuick/Layouts`
/// being `QtQuick.Layouts`. `QtQuick.Controls` is compiled from Qt's own
/// QML, so building it needs Qt; `Ljos.Design` is Ljós's QML, compiled as
/// a stand-in. A Ljós module written in Rust is in neither (DESIGN.md S6,
/// requests 32 to 34).
pub const WEB_MODULES: &[&str] = &[
    "Ljos.Blocks",
    "Ljos.Design",
    "Ljos.Models",
    "Qt.labs.folderlistmodel",
    "Qt.labs.qmlmodels",
    "Qt.labs.settings",
    "Qt.labs.synchronizer",
    "QtCharts",
    "QtCore",
    "QtGraphs",
    "QtLocation",
    "QtMultimedia",
    "QtPositioning",
    "QtQml",
    "QtQml.Models",
    "QtQml.XmlListModel",
    "QtQuick",
    "QtQuick.Controls",
    "QtQuick.Controls.Basic",
    "QtQuick.Controls.Basic.impl",
    "QtQuick.Controls.Fusion",
    "QtQuick.Controls.Fusion.impl",
    "QtQuick.Controls.Imagine",
    "QtQuick.Controls.Material",
    "QtQuick.Controls.Material.impl",
    "QtQuick.Controls.Universal",
    "QtQuick.Controls.Universal.impl",
    "QtQuick.Controls.impl",
    "QtQuick.Dialogs",
    "QtQuick.Effects",
    "QtQuick.Layouts",
    "QtQuick.LocalStorage",
    "QtQuick.Particles",
    "QtQuick.Shapes",
    "QtQuick.Templates",
    "QtQuick.Timeline",
    "QtQuick.VectorImage",
    "QtQuick.Window",
    "QtQuick3D",
    "QtQuick3D.Effects",
    "QtQuick3D.Helpers",
    "QtQuick3D.Helpers.impl",
    "QtQuick3D.Particles3D",
    "QtQuick3D.Physics",
    "QtQuick3D.Xr",
    "QtSensors",
    "QtWebSockets",
];
/// Bundles are written below this directory of the project, which is not
/// part of its documents (the store does not list it), so a bundle never
/// becomes part of the design nor of the next bundle.
const BUILD: &str = "build";
/// The project's mock modules (`preview_open`'s `mocked` state).
const MOCKS: &str = "mocks";

/// How many files and directories a bundle may hold through links it
/// cannot keep as links. Links that fan out and meet again copy a directory
/// once per way down, which doubles with every level, whether or not the
/// directories hold any files.
const LINKED_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    /// A directory `ljos run` starts.
    #[default]
    Native,
    /// A directory ljos-web builds a static site from.
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
    // A `///` here is bytes every session pays for in `tools/list`, which
    // has a cap: what else these take is in MCP.md.
    /// The first document.
    entry: String,
    #[serde(default)]
    target: Target,
    /// Below `build/`; replaced.
    out: Option<String>,
    // Says what the bundle would hold; writes nothing.
    #[serde(default)]
    dry_run: bool,
    // The site's title, description and pictures (web): written into the
    // project's `site.json`, which is read when this is not given.
    page: Option<Page>,
    // Build the site from the bundle with ljos-web, into `<out>-site`, and
    // serve it on this machine (web; needs `run`). With `dry_run`, only say
    // whether a build would find what it needs.
    #[serde(default)]
    build: bool,
    // Open the built site in the browser.
    #[serde(default)]
    open: bool,
    // The ljos-web folder to build with, when Studio does not find one.
    ljos_web: Option<String>,
    // The `qmlc` to build with, when Studio does not find one.
    qmlc: Option<String>,
}

impl ExportArgs {
    fn only_a_build_uses(&self) -> Option<&'static str> {
        [
            ("open", self.open),
            ("ljos_web", self.ljos_web.is_some()),
            ("qmlc", self.qmlc.is_some()),
        ]
        .into_iter()
        .find_map(|(argument, given)| given.then_some(argument))
    }
}

/// What a browser's tab, a search result and a shared link show of a site.
/// Its pictures are files of the site, by path relative to the project.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    // The tab's icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    // The picture a shared link shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

impl Page {
    fn pictures(&self) -> impl Iterator<Item = &str> {
        [self.icon.as_deref(), self.image.as_deref()]
            .into_iter()
            .flatten()
    }
}

/// What a bundle holds; written beside it as `ljos-bundle.json`, or for
/// the web as `ljos-site.json`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub target: Target,
    pub entry: String,
    /// The command that starts it, run in the bundle's directory (native).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Vec<String>>,
    /// The theme file the app reads: the nearest `theme.json` from its
    /// entry's directory up to the project's.
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
    /// The project's own modules and their `qmldir`s (web).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<Vec<ModuleDir>>,
    /// The modules the site imports that are not the project's, sorted:
    /// what the builder takes from ljos-web (web).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imports: Option<Vec<String>>,
    /// The ljos-web commit the imports were checked against (web).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ljos_web: Option<String>,
    /// The page's title, description and pictures (web).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ModuleDir {
    pub module: String,
    pub qmldir: String,
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
    /// `out` as an absolute path, which means the same wherever it is used.
    path: String,
    /// How many files the bundle holds and their size, which stay true when
    /// a long manifest is cut short in this answer (the one written is whole),
    /// copies of links it could not hold as links included.
    files: usize,
    bytes: u64,
    /// What a real export writes: the files and the manifest.
    writes: usize,
    manifest: Manifest,
    /// False for a dry run.
    written: bool,
    /// Whether `out` held a bundle of either kind before this call, which
    /// a real export replaces: what a person is told before they write it.
    replaces: bool,
    /// The export's own time; a build's steps say theirs.
    ms: f64,
    /// What `build` found, or built and where it is served.
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<BuildOut>,
}

#[derive(Serialize, JsonSchema, Default)]
pub struct BuildOut {
    /// Whether ljos-web's compiler, runtime and bundler and node were found.
    pub ready: bool,
    /// What a build lacks, in plain words.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ljos_web: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qmlc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// The built site's directory, relative to the project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// `site` as an absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Where this machine serves it, for a browser.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// What the build did, in order, and how long each part took.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<BuildStep>,
    /// Whether a browser was asked to open `url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opened: Option<bool>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct BuildStep {
    pub what: String,
    pub ms: f64,
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
    page: Option<&Page>,
) -> Result<Manifest, SurfaceError> {
    let store = studio.session.store();
    if !entry.ends_with(".qml") || !store.exists(entry) {
        return Err(SurfaceError::not_found(format!(
            "no document `{entry}` to start the app with"
        )));
    }
    let (files, links, left_out) = walk(root)?;
    // A site is plain files: its builder and the hosts it goes to keep no
    // links, so a linked directory is in it as a copy under the link's name.
    let (files, links, copies) = if target == Target::Web {
        let (files, copies) = copied_under_links(files, &links)?;
        (files, Vec::new(), copies)
    } else {
        (files, links, BTreeMap::new())
    };
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
    // or a screen of it that does not show. What each imports is kept for
    // a site, whose builder has to be told it.
    let mut imported: Vec<(&str, Vec<String>)> = Vec::new();
    for path in files
        .iter()
        .map(|f| f.path.as_str())
        .filter(|p| p.ends_with(".qml"))
    {
        let text = read(path)?;
        // Reading its imports parses it, and no more than that: the
        // objects' outline is not needed to know it parses.
        let Some(modules) = studio_core::outline::imports(&text) else {
            return Err(SurfaceError::new(
                ErrorCode::Syntax,
                format!("`{path}` does not parse; mend it before exporting"),
            )
            .with_details(json!({ "kind": "broken", "doc": path })));
        };
        let modules = modules
            .into_iter()
            .filter_map(|i| match i {
                studio_core::outline::Import::Module { uri, .. } => Some(uri),
                // A directory or script of the project's own is among its
                // files; one outside it is no more in a native bundle.
                studio_core::outline::Import::Path { .. } => None,
            })
            .collect();
        imported.push((path, modules));
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
            if target == Target::Web {
                "the project has types written in Rust, which only the app's own program \
                 registers; they have no face on the web yet (ljos-web request 33)"
            } else {
                "the project has types written in Rust, which only the app's own program \
                 registers; `ljos run` cannot start it"
            },
        )
        .with_details(json!({ "kind": "native_types", "files": native })));
    }

    let mut live = Vec::new();
    let mut module_dirs = Vec::new();
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
        // A site's builder finds modules by the qmldirs below it, so a copy
        // of a module's directory under a link's name would be a second
        // directory claiming the same module, and which one an import
        // reads would be the builder's guess.
        if let Some(of) = copies.get(path.as_str()) {
            return Err(SurfaceError::new(
                ErrorCode::Unsupported,
                format!(
                    "`{path}` is `{of}` again through a link, so the site would hold module \
                     {module} twice; import it by its name rather than through the link, or \
                     leave the link out of the project"
                ),
            )
            .with_details(json!({
                "kind": "linked_module",
                "module": module,
                "qmldir": of,
                "through": path,
            })));
        }
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        for (_, decl) in read_qmldir(&text).blocks {
            if let Some(file) = blocks::join(dir, &decl.file) {
                blocks_out.push(BlockFile {
                    block: format!("{module}.{}", decl.name),
                    file,
                });
            }
        }
        module_dirs.push(ModuleDir {
            module: module.clone(),
            qmldir: path.clone(),
        });
        live.push(module);
    }
    module_dirs.sort_by(|a, b| a.module.cmp(&b.module));
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

    let site = (target == Target::Web).then(|| web_imports(&imported, &live));
    let imports = match site {
        Some(Err(refusal)) => return Err(refusal),
        Some(Ok(imports)) => Some(imports),
        None => None,
    };

    let theme_path = crate::tools::theme_file(store, entry);
    let theme = files
        .iter()
        .any(|f| f.path == theme_path)
        .then_some(theme_path);
    let web = target == Target::Web;
    let (files, left_out, page) = if web {
        let page = match page {
            Some(page) => Some(page.clone()),
            None => site_page(root, &files)?,
        };
        let (files, left_out) = reached(root, files, left_out, theme.as_deref(), page.as_ref())?;
        (files, left_out, page)
    } else {
        (files, left_out, None)
    };
    let manifest = Manifest {
        format: if web { SITE_FORMAT } else { FORMAT }.to_owned(),
        version: if web { SITE_VERSION } else { VERSION },
        target,
        entry: entry.to_owned(),
        run: (!web).then(|| {
            ["ljos", "run", entry, "--project", "."]
                .map(str::to_owned)
                .to_vec()
        }),
        theme,
        blocks: blocks_out,
        files,
        links,
        left_out,
        modules: web.then_some(module_dirs),
        imports,
        ljos_web: web.then(|| LJOS_WEB.to_owned()),
        page,
    };
    Ok(manifest)
}

/// The project's page of its site, at its top.
pub const SITE_PAGE: &str = "site.json";
const PAGE_WHY: &str =
    "the page's title, description and pictures, which the manifest's page holds";
const UNNAMED_WHY: &str = "nothing the site's code names";

/// The page `site.json` gives, if the project has one among its files.
fn site_page(root: &Path, files: &[FileEntry]) -> Result<Option<Page>, SurfaceError> {
    if !files.iter().any(|f| f.path == SITE_PAGE) {
        return Ok(None);
    }
    let full = root.join(SITE_PAGE);
    let text = fs::read_to_string(&full).map_err(|e| io(&full, &e))?;
    serde_json::from_str(&text).map(Some).map_err(|e| {
        refused(
            ErrorCode::InvalidArgument,
            "page",
            format!("`{SITE_PAGE}` is not a page of the site ({e}); mend it or give `page`"),
        )
    })
}

/// Whether a file is the site's code, which is kept whole: a page may load
/// another by a name it makes, and the builder compiles what imports name.
fn is_code(path: &str) -> bool {
    let name = file_name(path);
    name == "qmldir"
        || name == "ljostypes.json"
        || [".qml", ".js", ".mjs", ".ts", ".mts"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

/// The site's files: its code, its theme, the page's pictures, and the files
/// its code names by a string (`"images/logo.svg"`, or a directory or the
/// start of a path a name is made from: `"icons/" + name + ".svg"`); every
/// other file is left out, saying why, as is `site.json`, which the
/// manifest's page holds. A picture of the page must be one of the files.
fn reached(
    root: &Path,
    files: Vec<FileEntry>,
    mut left_out: Vec<LeftOut>,
    theme: Option<&str>,
    page: Option<&Page>,
) -> Result<(Vec<FileEntry>, Vec<LeftOut>), SurfaceError> {
    if let Some(page) = page {
        for picture in page.pictures() {
            if picture == SITE_PAGE || !files.iter().any(|f| f.path == picture) {
                return Err(SurfaceError::new(
                    ErrorCode::InvalidArgument,
                    format!("the page's picture `{picture}` is not a file of the site"),
                )
                .with_details(json!({ "kind": "page_file", "path": picture })));
            }
        }
    }
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let mut keep: BTreeSet<String> = paths
        .iter()
        .copied()
        .filter(|p| is_code(p))
        .chain(theme)
        .chain(page.into_iter().flat_map(Page::pictures))
        .map(str::to_owned)
        .collect();
    for doc in paths.iter().copied().filter(|p| is_code(p)) {
        let full = root.join(doc);
        let text = fs::read_to_string(&full).map_err(|e| io(&full, &e))?;
        let dir = doc.rsplit_once('/').map_or("", |(dir, _)| dir);
        let named = if file_name(doc) == "qmldir" {
            text.split_whitespace().map(str::to_owned).collect()
        } else {
            literals(&text)
        };
        for literal in &named {
            keep.extend(
                named_by(&paths, dir, literal)
                    .into_iter()
                    .map(str::to_owned),
            );
        }
    }
    let (kept, unnamed): (Vec<FileEntry>, Vec<FileEntry>) = files
        .into_iter()
        .partition(|f| f.path != SITE_PAGE && keep.contains(f.path.as_str()));
    for file in unnamed {
        let why = if file.path == SITE_PAGE {
            PAGE_WHY
        } else {
            UNNAMED_WHY
        };
        left_out.push(LeftOut {
            path: file.path,
            why: why.to_owned(),
        });
    }
    left_out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((kept, left_out))
}

/// The files of `paths` a string in a document in `dir` names: the file it
/// is, or every file below the directory it is, or, for one with a `/` in
/// it, every file whose path starts with it. Read against the document's
/// directory and against the project's, as a QML `url` and a JavaScript
/// path are. Addresses elsewhere (`https://`, `data:`, `qrc:`) name none.
fn named_by<'p>(paths: &[&'p str], dir: &str, literal: &str) -> Vec<&'p str> {
    let literal = literal.split(['?', '#']).next().unwrap_or("");
    if literal.is_empty() || literal.contains(':') || literal.contains('\n') || literal.len() > 1024
    {
        return Vec::new();
    }
    // `"icons/"` names a directory; `"icons/" + name` the files it starts.
    let partial = literal.contains('/') && !literal.ends_with('/');
    let (literal, from_top) = match literal.strip_prefix('/') {
        Some(rest) => (rest, true),
        None => (literal, false),
    };
    let mut bases = vec![""];
    if !from_top && !dir.is_empty() {
        bases.insert(0, dir);
    }
    let mut named = Vec::new();
    for base in bases {
        let Some(at) = normal(base, literal) else {
            continue;
        };
        if at.is_empty() {
            continue;
        }
        let below = format!("{at}/");
        named.extend(paths.iter().copied().filter(|p| {
            *p == at || p.starts_with(&below) || (partial && p.starts_with(at.as_str()))
        }));
    }
    named
}

/// `literal` read from `base`, with `.` and `..` gone; `None` for a path out
/// of the project.
fn normal(base: &str, literal: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in literal.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// The strings of a QML or JavaScript text: `'...'`, `"..."` and the text
/// of a template between its `${...}`s, comments skipped. A regular
/// expression is read as code, which at worst keeps a file more.
fn literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    code(&chars, &mut at, &mut out, false);
    out
}

/// Reads code from `at` until its end, or, `nested`, the `}` that closes a
/// template's `${`.
fn code(chars: &[char], at: &mut usize, out: &mut Vec<String>, nested: bool) {
    let mut depth = 0usize;
    while let Some(&c) = chars.get(*at) {
        *at += 1;
        match c {
            '/' if chars.get(*at) == Some(&'/') => {
                while chars.get(*at).is_some_and(|&c| c != '\n') {
                    *at += 1;
                }
            }
            '/' if chars.get(*at) == Some(&'*') => {
                *at += 1;
                while *at < chars.len() && !(chars[*at] == '*' && chars.get(*at + 1) == Some(&'/'))
                {
                    *at += 1;
                }
                *at = (*at + 2).min(chars.len());
            }
            '\'' | '"' => out.push(quoted(chars, at, c)),
            '`' => template(chars, at, out),
            '{' => depth += 1,
            '}' if nested && depth == 0 => return,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
}

fn quoted(chars: &[char], at: &mut usize, quote: char) -> String {
    let mut text = String::new();
    while let Some(&c) = chars.get(*at) {
        *at += 1;
        match c {
            '\\' => {
                if let Some(&next) = chars.get(*at) {
                    *at += 1;
                    text.push(next);
                }
            }
            c if c == quote || c == '\n' => break,
            c => text.push(c),
        }
    }
    text
}

fn template(chars: &[char], at: &mut usize, out: &mut Vec<String>) {
    let mut text = String::new();
    while let Some(&c) = chars.get(*at) {
        *at += 1;
        match c {
            '\\' => {
                if let Some(&next) = chars.get(*at) {
                    *at += 1;
                    text.push(next);
                }
            }
            '`' => break,
            '$' if chars.get(*at) == Some(&'{') => {
                *at += 1;
                out.push(std::mem::take(&mut text));
                code(chars, at, out, true);
            }
            c => text.push(c),
        }
    }
    out.push(text);
}

/// `files` and, under each link's name, the files of the directory it
/// leads to: a site keeps no links, so it holds them as the native fallback
/// writes them where no link can be made, and refused past the same bound.
/// Also which file each copy is a copy of.
fn copied_under_links(
    mut files: Vec<FileEntry>,
    links: &[LinkEntry],
) -> Result<(Vec<FileEntry>, BTreeMap<String, String>), SurfaceError> {
    let all: Vec<&LinkEntry> = links.iter().collect();
    let (mut through, copies): (Vec<FileEntry>, BTreeMap<String, String>) =
        through_links(&files, links, &all)?
            .into_iter()
            .map(|(path, file)| {
                (
                    FileEntry {
                        path: path.clone(),
                        bytes: file.bytes,
                    },
                    (path, file.path.clone()),
                )
            })
            .unzip();
    files.append(&mut through);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((files, copies))
}

/// The modules a site imports from ljos-web, sorted; refused when a
/// document imports one that is neither the project's own nor ljos-web's,
/// which no site builder could compile.
fn web_imports(
    imported: &[(&str, Vec<String>)],
    live: &[String],
) -> Result<Vec<String>, SurfaceError> {
    let mut from_web = BTreeSet::new();
    let mut faceless: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (doc, modules) in imported {
        for module in modules {
            if live.contains(module) {
                continue;
            }
            if WEB_MODULES.contains(&module.as_str()) {
                from_web.insert(module.clone());
            } else {
                faceless.entry(module).or_default().insert(doc);
            }
        }
    }
    if faceless.is_empty() {
        return Ok(from_web.into_iter().collect());
    }
    let modules: Vec<&str> = faceless.keys().copied().collect();
    let docs: BTreeSet<&str> = faceless.values().flatten().copied().collect();
    Err(SurfaceError::new(
        ErrorCode::Unsupported,
        format!(
            "{} has no face on the web: neither the project nor ljos-web ({}) provides it; \
             export `native` to run it with `ljos run`",
            modules.join(", "),
            &LJOS_WEB[..LJOS_WEB.len().min(7)],
        ),
    )
    .with_details(json!({ "kind": "no_web_face", "modules": modules, "docs": docs })))
}

fn manifest_name(target: Target) -> &'static str {
    match target {
        Target::Native => MANIFEST,
        Target::Web => SITE_MANIFEST,
    }
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
    if folded(&path).starts_with(folded(Path::new(SITE_WORK))) {
        return Err(refused(
            ErrorCode::PathDenied,
            "work_directory",
            format!(
                "`{out}` is where Studio builds sites, emptied by each build; choose another `out`"
            ),
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
pub(crate) fn leads_elsewhere(root: &Path, full: &Path) -> bool {
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

/// Whether `dir` holds either kind of bundle's own manifest: a file, not
/// a link to one, that says it is that manifest. A file only named like
/// one is somebody's, and the directory with it; so is a site built from
/// a bundle.
fn holds_bundle(dir: &Path) -> bool {
    [(MANIFEST, FORMAT), (SITE_MANIFEST, SITE_FORMAT)]
        .iter()
        .any(|(name, format)| {
            let at = dir.join(name);
            fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_file())
                && fs::read_to_string(&at)
                    .ok()
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                    .is_some_and(|v| v.get("format").and_then(|f| f.as_str()) == Some(format))
        })
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
    // A build replaces a site whole, and what is inside it with it.
    if let Some(site) = out
        .ancestors()
        .skip(1)
        .take_while(|at| *at != root)
        .find(|at| fs::symlink_metadata(at.join(SITE_STAMP)).is_ok_and(|m| m.file_type().is_file()))
    {
        return Err(refused(
            ErrorCode::InvalidArgument,
            "inside_site",
            format!(
                "`{}` is inside the site built at `{}`; choose another `out`",
                out.display(),
                site.display()
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
    let at = staging.join(manifest_name(manifest.target));
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
/// followed again. Every link met and every file placed counts toward
/// [`LINKED_ENTRIES`], so links that only lead to more links are bounded
/// too; past it the copy is refused before anything is copied.
fn through_links<'a>(
    files: &'a [FileEntry],
    links: &[LinkEntry],
    copied: &[&LinkEntry],
) -> Result<Vec<(String, &'a FileEntry)>, SurfaceError> {
    struct Copy<'a, 'l, 'i> {
        files: &'i [&'a FileEntry],
        links: &'i [&'l LinkEntry],
        copying: Vec<&'l str>,
        done: BTreeSet<(String, &'l str)>,
        out: Vec<(String, &'a FileEntry)>,
        entries: usize,
    }
    /// The entries of `sorted` below `to`: every path starting `to/` lies
    /// between `to/` and `to0`, `0` being the byte after `/`.
    fn below<'s, T>(sorted: &'s [T], to: &str, path: impl Fn(&T) -> &str) -> &'s [T] {
        let (from, until) = (format!("{to}/"), format!("{to}0"));
        let start = sorted.partition_point(|e| path(e) < from.as_str());
        let end = sorted.partition_point(|e| path(e) < until.as_str());
        &sorted[start..end]
    }
    impl<'l> Copy<'_, 'l, '_> {
        fn count(&mut self) -> Result<(), SurfaceError> {
            if self.entries == LINKED_ENTRIES {
                return Err(refused(
                    ErrorCode::Unsupported,
                    "links_multiply",
                    format!(
                        "the project's links lead into the same directories along so many \
                         ways that copying them, where the bundle cannot hold them as links, \
                         would write more than {LINKED_ENTRIES} files and directories; link \
                         each directory once"
                    ),
                ));
            }
            self.entries += 1;
            Ok(())
        }
        fn link(&mut self, path: String, to: &'l str) -> Result<(), SurfaceError> {
            self.count()?;
            if self.copying.contains(&to) || !self.done.insert((path.clone(), to)) {
                return Ok(());
            }
            self.copying.push(to);
            let skip = to.len() + 1;
            let (files, links) = (self.files, self.links);
            for &file in below(files, to, |f| &f.path) {
                self.count()?;
                self.out
                    .push((format!("{path}/{}", &file.path[skip..]), file));
            }
            for inner in below(links, to, |l| &l.path) {
                self.link(format!("{path}/{}", &inner.path[skip..]), &inner.to)?;
            }
            self.copying.pop();
            Ok(())
        }
    }
    let mut files: Vec<&FileEntry> = files.iter().collect();
    let mut links: Vec<&LinkEntry> = links.iter().collect();
    files.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    links.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    let mut copy = Copy {
        files: &files,
        links: &links,
        copying: Vec::new(),
        done: BTreeSet::new(),
        out: Vec::new(),
        entries: 0,
    };
    for link in copied {
        copy.link(link.path.clone(), &link.to)?;
    }
    Ok(copy.out)
}

/// What a bundle's name has after it as the name of the site built from it.
pub(crate) const SITE_SUFFIX: &str = "-site";

/// Where a build works, below the project's `build/`.
pub(crate) const SITE_WORK: &str = "build/.site-work";

/// What the builder leaves at the root of a site it wrote.
const SITE_STAMP: &str = ".ljos-site-build";

/// The site built from `bundle`.
fn site_of(bundle: &Path) -> PathBuf {
    let mut site = bundle.as_os_str().to_owned();
    site.push(SITE_SUFFIX);
    PathBuf::from(site)
}

/// What builds read and write, or wait their turn to.
static BEING_BUILT: LazyLock<Mutex<Vec<PathBuf>>> = LazyLock::new(Mutex::default);

/// A path as holds compare it, without its letters' case: on a disk that
/// ignores it `build/WEB` is `build/web`. Where the two differ, an export
/// to one only waits for a build of the other.
fn folded(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

/// A build's hold on the bundle it reads and the directory it writes the
/// site to. The project is let go of while a site builds, so without it
/// another export could change what gets built, or take the site's place.
#[doc(hidden)]
pub struct BeingBuilt([PathBuf; 2]);

impl BeingBuilt {
    pub fn hold(bundle: PathBuf) -> Self {
        let bundle = folded(&bundle);
        let site = site_of(&bundle);
        let both = [bundle, site];
        BEING_BUILT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(both.clone());
        Self(both)
    }

    /// Whether replacing `bundle` would change what a build reads or writes.
    fn holds(bundle: &Path) -> bool {
        let bundle = folded(bundle);
        BEING_BUILT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|held| held.starts_with(&bundle) || bundle.starts_with(held))
    }
}

impl Drop for BeingBuilt {
    fn drop(&mut self) {
        let mut held = BEING_BUILT.lock().unwrap_or_else(PoisonError::into_inner);
        for ours in &self.0 {
            if let Some(at) = held.iter().position(|path| path == ours) {
                held.swap_remove(at);
            }
        }
    }
}

fn export(studio: &Mutex<Studio>, ctx: &CallCtx, a: ExportArgs) -> Result<ExportOut, SurfaceError> {
    let web = a.target == Target::Web;
    if !web && (a.build || a.page.is_some()) {
        let (what, kind) = if a.build {
            ("built", "build_target")
        } else {
            ("given a page", "page_target")
        };
        return Err(refused(
            ErrorCode::InvalidArgument,
            kind,
            format!("only a web export is {what}: give `\"target\": \"web\"`"),
        ));
    }
    // `tools/list` has no room to say that these go with `build`, and
    // ignoring them would answer a success and no site.
    if !a.build
        && let Some(argument) = a.only_a_build_uses()
    {
        let target = if web {
            ""
        } else {
            " and `\"target\": \"web\"`"
        };
        return Err(SurfaceError::new(
            ErrorCode::InvalidArgument,
            format!("`{argument}` needs `\"build\": true`{target}: only a build uses it"),
        )
        .with_details(json!({ "kind": "needs_build", "argument": argument })));
    }
    // Building runs ljos-web's compiler and bundler, which are programs.
    if a.build && !ctx.caps.contains(Cap::Run) {
        return Err(ljos_mcp_core::denied("export", Cap::Run, ctx));
    }
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
    let mut manifest = plan(&root, &studio, &a.entry, a.target, a.page.as_ref())?;
    let out = out_dir(&root, a.out.as_deref(), a.target)?;
    let path = root.join(&out);
    let replaces = holds_bundle(&path);
    let path = path.to_string_lossy().into_owned();
    // What a build would lack is said before anything is written.
    let tools = a.build.then(|| site_tools(&root, &a));
    if let Some(Err(missing)) = &tools
        && !a.dry_run
    {
        return Err(SurfaceError::new(
            ErrorCode::Unavailable,
            format!("Studio cannot build the site: {}", missing.join("; ")),
        )
        .with_details(json!({ "kind": "no_site_builder", "missing": missing })));
    }
    let copied = if a.dry_run {
        // What the write would refuse, the dry run says first.
        may_replace(&root, &root.join(&out))?;
        Copied::default()
    } else {
        let bundle = root.join(&out);
        // A build replaces its site, where the bundle of one waiting can lie.
        if BeingBuilt::holds(&bundle) || (a.build && BeingBuilt::holds(&site_of(&bundle))) {
            return Err(SurfaceError::new(
                ErrorCode::Busy,
                format!(
                    "`{out}`, or the site built from it, overlaps a site build that is running: export there again when it is done"
                ),
            )
            .with_details(json!({ "kind": "build_running" })));
        }
        if let Some(page) = &a.page {
            let mut text = serde_json::to_string_pretty(page).expect("a page is JSON");
            text.push('\n');
            studio
                .session
                .store()
                .write_bytes(SITE_PAGE, text.as_bytes())
                .map_err(crate::store_error)?;
            if !manifest.left_out.iter().any(|l| l.path == SITE_PAGE) {
                manifest.left_out.push(LeftOut {
                    path: SITE_PAGE.to_owned(),
                    why: PAGE_WHY.to_owned(),
                });
                manifest.left_out.sort_by(|a, b| a.path.cmp(&b.path));
            }
        }
        write(&root, &root.join(&out), &manifest)?
    };
    let ms = studio_core::preview::ms(studio.timer.now().saturating_sub(started));
    // A build takes as long as ljos-web does; the project is not held for
    // it, only the bundle and the site, from before anyone else can export.
    let _held = (a.build && !a.dry_run).then(|| BeingBuilt::hold(root.join(&out)));
    drop(studio);
    let build = match tools {
        None => None,
        Some(lookup) if a.dry_run => Some(match lookup {
            Ok(tools) => found(&tools),
            Err(missing) => BuildOut {
                missing,
                ..BuildOut::default()
            },
        }),
        Some(Ok(tools)) => Some(build_site(&root, &out, &tools, a.open)?),
        Some(Err(_)) => unreachable!("refused above"),
    };
    Ok(ExportOut {
        out,
        path,
        files: manifest.files.len() + copied.files,
        bytes: manifest.files.iter().map(|f| f.bytes).sum::<u64>() + copied.bytes,
        writes: manifest.files.len() + copied.files + 1,
        manifest,
        written: !a.dry_run,
        replaces,
        ms,
        build,
    })
}

#[cfg(feature = "native")]
type Tools = crate::site_build::Toolchain;
#[cfg(not(feature = "native"))]
type Tools = std::convert::Infallible;

#[cfg(feature = "native")]
fn site_tools(root: &Path, a: &ExportArgs) -> Result<Tools, Vec<String>> {
    crate::site_build::find(root, a.ljos_web.as_deref(), a.qmlc.as_deref())
}

#[cfg(not(feature = "native"))]
fn site_tools(_: &Path, a: &ExportArgs) -> Result<Tools, Vec<String>> {
    // A browser's Studio is given no folder to build with.
    let _ = (&a.ljos_web, &a.qmlc);
    Err(vec![
        "a site is built only where Studio runs on the machine, not in a browser".to_owned(),
    ])
}

#[cfg(feature = "native")]
fn found(tools: &Tools) -> BuildOut {
    let at = |p: &Path| Some(p.to_string_lossy().into_owned());
    BuildOut {
        ready: true,
        ljos_web: at(&tools.ljos_web),
        qmlc: at(&tools.qmlc),
        node: at(&tools.node),
        ..BuildOut::default()
    }
}

#[cfg(not(feature = "native"))]
fn found(tools: &Tools) -> BuildOut {
    match *tools {}
}

#[cfg(feature = "native")]
fn build_site(root: &Path, out: &str, tools: &Tools, open: bool) -> Result<BuildOut, SurfaceError> {
    crate::site_build::build(root, out, tools, open)
}

#[cfg(not(feature = "native"))]
fn build_site(_: &Path, _: &str, tools: &Tools, _: bool) -> Result<BuildOut, SurfaceError> {
    match *tools {}
}

pub(crate) fn add(registry: ToolRegistry, studio: &Arc<Mutex<Studio>>) -> ToolRegistry {
    let studio = studio.clone();
    registry.with(
        ToolDef::new::<ExportArgs>(
            "export",
            "Write a bundle that runs without Studio, never mocks or `.studio/`: for `ljos run`, or `web` to `build` with ljos-web. `dry_run` only checks.",
        )
        .answers::<ExportOut>()
        .requires(Cap::Fs)
        .annotated(ToolAnnotations {
            read_only: false,
            destructive: true,
            idempotent: true,
            open_world: false,
        }),
        typed(move |ctx, a: ExportArgs| export(&studio, ctx, a)),
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
            run: None,
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
            modules: None,
            imports: None,
            ljos_web: None,
            page: None,
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

    /// `levels` directories under `levels/`, each with a `level.txt` if
    /// `with_files` and, but the last, links `left` and `right` to the next.
    fn diamond(root: &Path, levels: usize, with_files: bool) {
        for level in 0..levels {
            let dir = root.join("levels").join(level.to_string());
            fs::create_dir_all(&dir).unwrap();
            if with_files {
                fs::write(dir.join("level.txt"), format!("{level}\n")).unwrap();
            }
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
        diamond(&root, 24, true);
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
    fn a_diamond_of_links_to_directories_without_files_is_refused_before_anything_is_written() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        diamond(&root, 24, false);
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
        diamond(&root, 3, true);
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

    #[test]
    fn the_strings_of_code_are_read_and_its_comments_are_not() {
        let text = r#"
            // "commented.png"
            /* 'also.png' */
            Image { source: "a.png"; property string b: 'b\'s.png' }
            function f(n) { return `icons/${n ? "on" : 'off'}.svg` }
            var url = "https://example.com/x.png" // "after.png"
        "#;
        assert_eq!(
            literals(text),
            [
                "a.png",
                "b's.png",
                "icons/",
                "on",
                "off",
                ".svg",
                "https://example.com/x.png"
            ]
        );
    }

    #[test]
    fn a_string_names_a_file_a_directory_or_the_start_of_a_path() {
        let paths = [
            "Pages/Home.qml",
            "Pages/hero.png",
            "icons/play.svg",
            "icons/stop.svg",
            "images/logo.svg",
            "images/logo.png",
            "iconsets/x.svg",
        ];
        assert_eq!(named_by(&paths, "Pages", "hero.png"), ["Pages/hero.png"]);
        assert_eq!(
            named_by(&paths, "Pages", "../images/logo.svg"),
            ["images/logo.svg"]
        );
        assert_eq!(
            named_by(&paths, "Pages", "/icons/play.svg?v=2"),
            ["icons/play.svg"]
        );
        assert_eq!(
            named_by(&paths, "", "icons/"),
            ["icons/play.svg", "icons/stop.svg"]
        );
        assert_eq!(
            named_by(&paths, "", "images/logo"),
            ["images/logo.svg", "images/logo.png"]
        );
        assert!(named_by(&paths, "", "icons").len() == 2);
        assert!(named_by(&paths, "", "../outside.png").is_empty());
        assert!(named_by(&paths, "", "qrc:/icons/play.svg").is_empty());
        assert!(named_by(&paths, "", "logo").is_empty());
        assert!(named_by(&paths, "", "").is_empty());
    }

    #[test]
    fn a_site_keeps_its_code_its_theme_its_pictures_and_what_is_named() {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let write = |path: &str, text: &str| {
            let at = root.join(path);
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            fs::write(at, text).unwrap();
        };
        write("Shell.qml", "Image { source: \"images/logo.svg\" }");
        write("lib/util.js", "// nothing named");
        write("images/logo.svg", "");
        write("images/icon.png", "");
        write("images/unused.svg", "");
        write("theme.json", "{}");
        write("site.json", "{}");
        let (files, _, left_out) = walk(&root).unwrap();
        let page = Page {
            icon: Some("images/icon.png".to_owned()),
            ..Page::default()
        };
        let (kept, left_out) = reached(
            &root,
            files.clone(),
            left_out,
            Some("theme.json"),
            Some(&page),
        )
        .unwrap();
        let kept: Vec<&str> = kept.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            kept,
            [
                "Shell.qml",
                "images/icon.png",
                "images/logo.svg",
                "lib/util.js",
                "theme.json"
            ]
        );
        let left: Vec<(&str, &str)> = left_out
            .iter()
            .map(|l| (l.path.as_str(), l.why.as_str()))
            .collect();
        assert_eq!(
            left,
            [("images/unused.svg", UNNAMED_WHY), ("site.json", PAGE_WHY)]
        );
        let elsewhere = Page {
            image: Some("../secret.png".to_owned()),
            ..Page::default()
        };
        let refusal = reached(&root, files, Vec::new(), None, Some(&elsewhere)).unwrap_err();
        assert_eq!(refusal.to_value()["error"]["details"]["kind"], "page_file");
    }
}
