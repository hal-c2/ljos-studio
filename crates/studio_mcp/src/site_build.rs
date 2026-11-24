//! Building a web export into a site, and serving it on this machine
//! (DESIGN.md S6, as built: Studio builds the site).
//!
//! Studio carries its own builder (`web/site/`): a Vite build with
//! ljos-web's plugin, which gives the page its head and lets it scroll as a
//! document. What it does not carry is ljos-web itself, which has no package
//! to install and whose compiler is a native program: Studio finds a
//! checkout of it with its packages installed, and its `qmlc` (DECISION in
//! DESIGN.md S6). Nothing is downloaded.
//!
//! A built site is served over loopback, since a browser runs no module
//! script from a `file://` page.

use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        LazyLock, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

use ljos_design::module;
use ljos_mcp_core::{ErrorCode, SurfaceError};
use serde_json::json;

use crate::export::{BuildOut, BuildStep, SITE_WORK};

/// What a build runs: an ljos-web checkout whose packages are installed,
/// its compiler and node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toolchain {
    pub ljos_web: PathBuf,
    pub qmlc: PathBuf,
    pub node: PathBuf,
}

/// ljos-web's packages the builder imports, below its example app's
/// `node_modules`, where `pnpm install` puts them.
const PACKAGES: &[&str] = &["ljos-web", "solid-js", "vite", "@solidjs/web"];
const MODULES: &str = "examples/web/node_modules";

/// The builder, carried in Studio so a person who installed it needs no
/// checkout of it.
const BUILDER: &[(&str, &str)] = &[
    ("build.mjs", include_str!("../../../web/site/build.mjs")),
    ("index.html", include_str!("../../../web/site/index.html")),
    ("main.js", include_str!("../../../web/site/main.js")),
    (
        "vite.config.js",
        include_str!("../../../web/site/vite.config.js"),
    ),
];

/// Where `vite.config.js` looks for the QML modules Studio gives the builder.
const STANDINS: &str = "standins";

/// `Ljos.Design` as Ljós builds it in, which ljos-web has no QML of: each
/// file by its path in the work directory, and its text.
fn design() -> impl Iterator<Item = (PathBuf, String)> {
    let dir = Path::new(STANDINS).join(module::DIRECTORY);
    module::files(module::Seeds::Hues)
        .into_iter()
        .map(move |file| (dir.join(file.name), file.text))
}

/// What a build needs, or what of it is missing in plain words. `ljos_web`
/// and `qmlc` are a person's choice, relative to the project; without them
/// Studio looks where they are kept.
pub fn find(
    project: &Path,
    ljos_web: Option<&str>,
    qmlc: Option<&str>,
) -> Result<Toolchain, Vec<String>> {
    let mut missing = Vec::new();
    let web = find_ljos_web(project, ljos_web);
    let web = web.map_err(|why| missing.push(why)).ok();
    let compiler = find_qmlc(project, qmlc, web.as_deref());
    let compiler = compiler.map_err(|why| missing.push(why)).ok();
    let node = env_or_path("NODE", "node");
    if node.is_none() {
        missing.push(
            "no node found: install Node.js 22 or newer, or set NODE to the program".to_owned(),
        );
    }
    match (web, compiler, node) {
        (Some(ljos_web), Some(qmlc), Some(node)) => Ok(Toolchain {
            ljos_web,
            qmlc,
            node,
        }),
        _ => Err(missing),
    }
}

fn find_ljos_web(project: &Path, given: Option<&str>) -> Result<PathBuf, String> {
    if let Some(given) = given {
        return ready(&project.join(given));
    }
    if let Some(env) = std::env::var_os("LJOS_WEB") {
        return ready(Path::new(&env));
    }
    ljos_web_in(ljos_web_places(project))
}

/// The first of `places` that is an ljos-web ready to build with. A folder
/// of that name that is not one, or not installed, does not hide one
/// further on; when none is ready, what the first lacks is said.
fn ljos_web_in(places: Vec<PathBuf>) -> Result<PathBuf, String> {
    let mut looked = Vec::new();
    let mut first = None;
    for dir in places {
        if dir.is_dir() {
            match ready(&dir) {
                Ok(dir) => return Ok(dir),
                Err(why) => {
                    first.get_or_insert(why);
                }
            }
        }
        looked.push(dir.to_string_lossy().into_owned());
    }
    if let Some(why) = first {
        return Err(why);
    }
    Err(format!(
        "no ljos-web found: choose its folder or set LJOS_WEB (looked in {})",
        looked.join(", ")
    ))
}

/// Where an ljos-web is kept when nobody says: beside Studio's program, in
/// its share directory, beside the project, and in the user's data.
fn ljos_web_places(project: &Path) -> Vec<PathBuf> {
    let mut places = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_owned))
    {
        places.push(dir.join("ljos-web"));
        places.push(dir.join("../share/ljos-web"));
    }
    if let Some(parent) = project.parent() {
        places.push(parent.join("ljos-web"));
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")));
    if let Some(data) = data {
        places.push(data.join("ljos-web"));
    }
    places
}

/// `dir` when it is an ljos-web whose packages are installed.
fn ready(dir: &Path) -> Result<PathBuf, String> {
    if !dir.is_dir() {
        return Err(format!(
            "no ljos-web at {}: choose its folder",
            dir.display()
        ));
    }
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    if !dir.join("packages/runtime/package.json").is_file() {
        return Err(format!(
            "{} is not an ljos-web: it has no packages/runtime",
            dir.display()
        ));
    }
    let lacking: Vec<&str> = PACKAGES
        .iter()
        .copied()
        .filter(|p| !dir.join(MODULES).join(p).join("package.json").is_file())
        .collect();
    if !lacking.is_empty() {
        return Err(format!(
            "the ljos-web at {} has not got {} installed: run `pnpm install` in it",
            dir.display(),
            lacking.join(", ")
        ));
    }
    Ok(dir)
}

fn find_qmlc(project: &Path, given: Option<&str>, web: Option<&Path>) -> Result<PathBuf, String> {
    let exe = |dir: &Path| dir.join(format!("qmlc{}", std::env::consts::EXE_SUFFIX));
    if let Some(given) = given {
        let at = project.join(given);
        return at
            .is_file()
            .then_some(at.clone())
            .ok_or_else(|| format!("no qmlc at {}: choose the program", at.display()));
    }
    if let Some(env) = std::env::var_os("QMLC") {
        let at = PathBuf::from(env);
        return at
            .is_file()
            .then_some(at.clone())
            .ok_or_else(|| format!("QMLC says {}, which is no program", at.display()));
    }
    let mut places = Vec::new();
    if let Some(web) = web {
        places.push(exe(&web.join("target/release")));
        places.push(exe(&web.join("target/debug")));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_owned))
    {
        places.push(exe(&dir));
    }
    places
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| on_path("qmlc"))
        .ok_or_else(|| {
            "no qmlc found: build it in ljos-web (`cargo build --release -p qmlc`), choose it, \
             or set QMLC"
                .to_owned()
        })
}

fn env_or_path(var: &str, name: &str) -> Option<PathBuf> {
    match std::env::var_os(var) {
        Some(at) => Some(PathBuf::from(at)).filter(|p| p.is_file()),
        None => on_path(name),
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    let name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&name))
        .find(|p| p.is_file())
}

/// One build at a time: they share the work directory.
static BUILDING: Mutex<()> = Mutex::new(());

/// A build refused. `log` is `None` when it stopped before the builder
/// wrote one: the log there is the last build's.
fn failed(
    step: &str,
    why: impl Into<String>,
    log: Option<&Path>,
    steps: &[BuildStep],
) -> SurfaceError {
    let why = why.into();
    SurfaceError::new(
        ErrorCode::NotActionable,
        format!("the site was not built: {why}"),
    )
    .with_details(json!({
        "kind": "build_failed",
        "step": step,
        "why": why,
        "log": log.map(|l| l.to_string_lossy()),
        "steps": steps,
    }))
}

/// How long a build may take before it is stopped.
const BUILD_DEADLINE: Duration = Duration::from_secs(15 * 60);

/// Builds the export at `out` (below the project's `build/`) into
/// `<out>-site`, serves it, and opens it when asked.
pub fn build(
    root: &Path,
    out: &str,
    tools: &Toolchain,
    open: bool,
) -> Result<BuildOut, SurfaceError> {
    build_within(root, out, tools, open, BUILD_DEADLINE)
}

/// [`build`], stopped when it takes longer than `deadline`.
fn build_within(
    root: &Path,
    out: &str,
    tools: &Toolchain,
    open: bool,
    deadline: Duration,
) -> Result<BuildOut, SurfaceError> {
    let _one = BUILDING.lock().unwrap_or_else(|e| e.into_inner());
    let site = format!("{out}{}", crate::export::SITE_SUFFIX);
    let work = root.join(SITE_WORK);
    let log = work.join("build.log");
    for at in std::iter::once(root.join(&site)).chain(written_in(&work)) {
        if crate::export::leads_elsewhere(root, &at) {
            return Err(SurfaceError::new(
                ErrorCode::PathDenied,
                format!(
                    "`{}` goes through a link; sites are built in the project's own `build/`",
                    at.display()
                ),
            ));
        }
    }
    let setup = "Getting the builder ready";
    prepare(&work, tools).map_err(|why| failed(setup, why, None, &[]))?;
    let log_file = fs::File::create(&log).map_err(|e| failed(setup, e.to_string(), None, &[]))?;
    let mut command = Command::new(&tools.node);
    command
        .arg(work.join("build.mjs"))
        .arg(root.join(out))
        .arg(root.join(&site))
        .current_dir(&work)
        .env("QMLC", &tools.qmlc)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log_file);
    let types = tools.ljos_web.join("crates/ljos_web/types.txt");
    if std::env::var_os("LJOS_QT_TYPES").is_none() && types.is_file() {
        command.env("LJOS_QT_TYPES", types);
    }
    // A group of its own, so what it starts is stopped with it.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .map_err(|e| failed(setup, format!("node did not start: {e}"), Some(&log), &[]))?;
    let said = BufReader::new(child.stdout.take().expect("piped"));
    // Read on a thread of its own, so a builder that hangs without a word
    // is still stopped at the deadline.
    let (tx, lines) = mpsc::channel();
    let reader = std::thread::Builder::new()
        .name("site build output".to_owned())
        .spawn(move || {
            for line in said.lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    if let Err(e) = reader {
        stop(&mut child);
        return Err(failed(setup, e.to_string(), Some(&log), &[]));
    }
    let started = Instant::now();
    let mut steps: Vec<BuildStep> = Vec::new();
    let mut since = Instant::now();
    let mut bytes = None;
    let mut why = None;
    loop {
        let line = match lines.recv_timeout(deadline.saturating_sub(started.elapsed())) {
            Ok(line) => line,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                stop(&mut child);
                if let Some(last) = steps.last_mut() {
                    last.ms = since.elapsed().as_secs_f64() * 1000.0;
                }
                let step = steps.last().map_or(setup, |s| s.what.as_str()).to_owned();
                let why = format!(
                    "it took longer than {} and was stopped; its log says how far it got",
                    span(deadline)
                );
                return Err(failed(&step, why, Some(&log), &steps));
            }
        };
        let close = |steps: &mut Vec<BuildStep>, since: Instant| {
            if let Some(last) = steps.last_mut() {
                last.ms = since.elapsed().as_secs_f64() * 1000.0;
            }
        };
        if let Some(what) = line.strip_prefix("step ") {
            close(&mut steps, since);
            since = Instant::now();
            steps.push(BuildStep {
                what: what.to_owned(),
                ms: 0.0,
            });
        } else if let Some(n) = line.strip_prefix("done ") {
            close(&mut steps, since);
            bytes = n.trim().parse().ok();
        } else if let Some(what) = line.strip_prefix("fail ") {
            close(&mut steps, since);
            why = Some(what.to_owned());
        }
    }
    let status = child.wait();
    let step = steps.last().map_or(setup, |s| s.what.as_str()).to_owned();
    match (status, why, bytes) {
        (_, Some(why), _) => return Err(failed(&step, why, Some(&log), &steps)),
        (Ok(status), None, Some(_)) if status.success() => {}
        (Ok(status), None, _) => {
            let why =
                format!("the builder stopped ({status}) without saying why; its log says more");
            return Err(failed(&step, why, Some(&log), &steps));
        }
        (Err(e), None, _) => return Err(failed(&step, e.to_string(), Some(&log), &steps)),
    }
    let path = root.join(&site);
    let url = serve(&path).map_err(|e| {
        failed(
            "Serving the site",
            format!("it was built, but not served: {e}"),
            Some(&log),
            &steps,
        )
    })?;
    let opened = open.then(|| browse(&url));
    Ok(BuildOut {
        ready: true,
        ljos_web: Some(tools.ljos_web.to_string_lossy().into_owned()),
        qmlc: Some(tools.qmlc.to_string_lossy().into_owned()),
        node: Some(tools.node.to_string_lossy().into_owned()),
        site: Some(site),
        path: Some(path.to_string_lossy().into_owned()),
        url: Some(url),
        bytes,
        steps,
        opened,
        ..BuildOut::default()
    })
}

/// Stops a builder and all it started.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// A length of time in plain words.
fn span(time: Duration) -> String {
    let secs = time.as_secs();
    match secs {
        0..60 => format!("{secs} s"),
        _ => format!("{} minutes", secs / 60),
    }
}

/// What a build writes or empties in `work`. A project may come with any of
/// them made a link, and writing or emptying through one reaches out of it.
/// The packages are links of Studio's own, replaced without being followed.
fn written_in(work: &Path) -> Vec<PathBuf> {
    let names = [
        "build.log",
        "site",
        "dist",
        "node_modules/@solidjs",
        "node_modules/.vite",
        "node_modules/.vite-temp",
    ];
    names
        .into_iter()
        .chain(BUILDER.iter().map(|(name, _)| *name))
        .map(|name| work.join(name))
        .chain(design().map(|(name, _)| work.join(name)))
        .collect()
}

/// The builder's files and ljos-web's packages in the work directory.
fn prepare(work: &Path, tools: &Toolchain) -> Result<(), String> {
    let plain = |e: std::io::Error, at: &Path| format!("{}: {e}", at.display());
    fs::create_dir_all(work.join("node_modules/@solidjs")).map_err(|e| plain(e, work))?;
    for (name, text) in BUILDER {
        let at = work.join(name);
        fs::write(&at, text).map_err(|e| plain(e, &at))?;
    }
    let modules = work.join(STANDINS).join(module::DIRECTORY);
    fs::create_dir_all(&modules).map_err(|e| plain(e, &modules))?;
    for (name, text) in design() {
        let at = work.join(name);
        fs::write(&at, text).map_err(|e| plain(e, &at))?;
    }
    for package in PACKAGES {
        let at = work.join("node_modules").join(package);
        // Linked to where the package really is, so what it imports is
        // found beside it as pnpm laid it out.
        let to = tools.ljos_web.join(MODULES).join(package);
        let to = to.canonicalize().map_err(|e| plain(e, &to))?;
        if fs::symlink_metadata(&at).is_ok() {
            fs::remove_file(&at)
                // Windows removes a link to a directory as a directory. One
                // that holds files is not Studio's link, and stays.
                .or_else(|_| fs::remove_dir(&at))
                .map_err(|e| plain(e, &at))?;
        }
        link_dir(&to, &at).map_err(|e| {
            format!(
                "Studio could not link ljos-web's {package} into {}: {e}",
                work.display()
            )
        })?;
    }
    Ok(())
}

fn link_dir(to: &Path, at: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(to, at)
    }
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

/// The sites served, by directory, and their ports: one server for each,
/// for as long as Studio runs, so a site built again keeps its address.
static SERVED: LazyLock<Mutex<HashMap<PathBuf, u16>>> = LazyLock::new(Mutex::default);

/// The address this machine serves `site` on, which nobody else can reach.
pub fn serve(site: &Path) -> std::io::Result<String> {
    let mut served = SERVED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(port) = served.get(site) {
        return Ok(format!("http://127.0.0.1:{port}/"));
    }
    // The directory it is now: a link put in its place later leads nowhere
    // the server reads.
    let root = site.canonicalize()?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let dir = root;
    std::thread::Builder::new()
        .name("site server".to_owned())
        .spawn(move || {
            for stream in listener.incoming().map_while(Result::ok) {
                let dir = dir.clone();
                let _ = std::thread::Builder::new()
                    .name("site request".to_owned())
                    .spawn(move || answer(stream, &dir, port));
            }
        })?;
    served.insert(site.to_owned(), port);
    Ok(format!("http://127.0.0.1:{port}/"))
}

fn answer(mut stream: TcpStream, site: &Path, port: u16) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let mut head = Vec::new();
    let mut chunk = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => head.extend_from_slice(&chunk[..n]),
        }
        if head.len() > 16 * 1024 {
            return reply(
                &mut stream,
                "431 Request Header Fields Too Large",
                None,
                false,
            );
        }
    }
    let head = String::from_utf8_lossy(&head);
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let host = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim());
    // A page elsewhere that names this port by a name of its own (DNS
    // rebinding) reads nothing.
    if !host.is_some_and(|h| local_host(h, port)) {
        return reply(&mut stream, "403 Forbidden", None, false);
    }
    let body = method == "GET";
    if !body && method != "HEAD" {
        return reply(&mut stream, "405 Method Not Allowed", None, false);
    }
    let file = resolve(site, target).and_then(|at| {
        let file = fs::File::open(&at).ok()?;
        let len = file.metadata().ok()?.len();
        Some((at, file, len))
    });
    match file {
        Some((at, file, len)) => reply(&mut stream, "200 OK", Some((&at, file, len)), body),
        None => reply(&mut stream, "404 Not Found", None, false),
    }
}

fn local_host(host: &str, port: u16) -> bool {
    let host = host.to_ascii_lowercase();
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

/// Answers with `file`, its length said and its bytes copied as they are
/// read, so a large one is not held whole.
fn reply(stream: &mut TcpStream, status: &str, file: Option<(&Path, fs::File, u64)>, body: bool) {
    let (kind, len, file) = match file {
        Some((at, file, len)) => (kind(at), len, Some(file)),
        None => ("text/plain; charset=utf-8", 0, None),
    };
    // A picture is a document of its own when opened by its address; one
    // brought in from anywhere runs no script with the site's origin.
    let policy = if kind == "image/svg+xml" {
        "Content-Security-Policy: sandbox; default-src 'none'; script-src 'none'; \
         style-src 'unsafe-inline'; img-src data:\r\n"
    } else {
        ""
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{policy}\
         Cache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        len
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    if let (true, Some(file)) = (body, file) {
        let _ = std::io::copy(&mut file.take(len), stream);
    }
}

/// The file of `site`, a directory's real path, that a request's target
/// names, or `None`: a path out of it, a hidden name (the build's stamp) or
/// nothing there.
fn resolve(site: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next()?;
    let path = path.strip_prefix('/')?;
    let decoded = percent_decode(path)?;
    let mut at = site.to_owned();
    for part in decoded.split('/') {
        if part.is_empty() {
            continue;
        }
        if part.starts_with('.') || part.contains('\\') {
            return None;
        }
        at.push(part);
    }
    if !Path::new(&decoded)
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    if at.is_dir() {
        at.push("index.html");
    }
    let real = at.canonicalize().ok()?;
    (real.starts_with(site) && real.is_file()).then_some(real)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    let out = String::from_utf8(out).ok()?;
    (!out.contains('\0')).then_some(out)
}

fn kind(at: &Path) -> &'static str {
    match at.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Asks the person's browser to open `url`: `$BROWSER` when set, else
/// the system's opener. Whether it started is all that is known.
fn browse(url: &str) -> bool {
    let mut command = match std::env::var("BROWSER")
        .ok()
        .filter(|b| !b.trim().is_empty())
    {
        Some(browser) => {
            let first = browser.split(':').next().unwrap_or("").to_owned();
            let mut words = first.split_whitespace().map(str::to_owned);
            let Some(program) = words.next() else {
                return false;
            };
            let mut command = Command::new(program);
            let mut named = false;
            for word in words {
                named |= word.contains("%s");
                command.arg(word.replace("%s", url));
            }
            if !named {
                command.arg(url);
            }
            command
        }
        None if cfg!(target_os = "macos") => {
            let mut command = Command::new("open");
            command.arg(url);
            command
        }
        None if cfg!(windows) => {
            let mut command = Command::new("cmd");
            command.args(["/C", "start", "", url]);
            command
        }
        None => {
            let mut command = Command::new("xdg-open");
            command.arg(url);
            command
        }
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        Ok(mut child) => {
            // Reaped aside, so no opener is left a zombie.
            let _ = std::thread::Builder::new()
                .name("site opener".to_owned())
                .spawn(move || child.wait());
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let site = dir.path().join("site");
        fs::create_dir_all(site.join("assets")).unwrap();
        fs::write(site.join("index.html"), "<!doctype html>").unwrap();
        fs::write(site.join("assets/a b.js"), "1").unwrap();
        fs::write(site.join(".ljos-site-build"), "x").unwrap();
        fs::write(dir.path().join("secret.txt"), "no").unwrap();
        (dir, site)
    }

    #[test]
    fn a_request_reads_only_files_of_the_site() {
        let (_dir, site) = site();
        let real = site.canonicalize().unwrap();
        let site = real.clone();
        assert_eq!(resolve(&site, "/"), Some(real.join("index.html")));
        assert_eq!(resolve(&site, "/?x=1"), Some(real.join("index.html")));
        assert_eq!(
            resolve(&site, "/assets/a%20b.js"),
            Some(real.join("assets/a b.js"))
        );
        assert_eq!(resolve(&site, "/../secret.txt"), None);
        assert_eq!(resolve(&site, "/%2e%2e/secret.txt"), None);
        assert_eq!(resolve(&site, "/assets/..%2f..%2fsecret.txt"), None);
        assert_eq!(resolve(&site, "/.ljos-site-build"), None);
        assert_eq!(resolve(&site, "/nothing.js"), None);
        assert_eq!(resolve(&site, "secret.txt"), None);
        assert_eq!(resolve(&site, "/%zz"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_site_is_not_followed() {
        let (dir, site) = site();
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), site.join("out.txt")).unwrap();
        assert_eq!(resolve(&site, "/out.txt"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_build_writes_through_no_link_in_its_work_directory() {
        let dir = tempfile::tempdir().unwrap();
        let tools = Toolchain {
            ljos_web: dir.path().join("none"),
            qmlc: dir.path().join("none"),
            node: dir.path().join("none"),
        };
        for at in written_in(Path::new(SITE_WORK)) {
            let root = tempfile::tempdir().unwrap();
            let theirs = tempfile::tempdir().unwrap();
            let link = root.path().join(&at);
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(theirs.path(), &link).unwrap();
            let Err(refused) = build(root.path(), "build/web", &tools, false) else {
                panic!("built through the link at {}", at.display());
            };
            assert_eq!(refused.code, ErrorCode::PathDenied, "{}", at.display());
            assert_eq!(fs::read_dir(theirs.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn only_this_machine_by_its_own_names_is_answered() {
        assert!(local_host("127.0.0.1:8123", 8123));
        assert!(local_host("LOCALHOST:8123", 8123));
        assert!(!local_host("127.0.0.1:8124", 8123));
        assert!(!local_host("evil.example:8123", 8123));
        assert!(!local_host("127.0.0.1", 8123));
    }

    #[test]
    fn a_served_site_answers_over_loopback_and_keeps_its_address() {
        let (_dir, site) = site();
        let url = serve(&site).unwrap();
        assert_eq!(serve(&site).unwrap(), url);
        let port: u16 = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let ask = |request: String| {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut said = String::new();
            stream.read_to_string(&mut said).unwrap();
            said
        };
        let page = ask(format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
        assert!(page.contains("Content-Type: text/html"), "{page}");
        assert!(page.ends_with("<!doctype html>"), "{page}");
        let elsewhere = ask(format!(
            "GET / HTTP/1.1\r\nHost: evil.example:{port}\r\n\r\n"
        ));
        assert!(elsewhere.starts_with("HTTP/1.1 403"), "{elsewhere}");
        let post = ask(format!("POST / HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"));
        assert!(post.starts_with("HTTP/1.1 405"), "{post}");
    }

    // A picture imported from anywhere is served as it came: opened by
    // its address it is a document, and its script must not run as the
    // site's.
    #[test]
    fn a_served_svg_runs_no_script() {
        let (_dir, site) = site();
        fs::write(
            site.join("assets/logo.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#,
        )
        .unwrap();
        let url = serve(&site).unwrap();
        let port: u16 = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let ask = |path: &str| {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
            stream.write_all(request.as_bytes()).unwrap();
            let mut said = String::new();
            stream.read_to_string(&mut said).unwrap();
            said
        };
        let svg = ask("/assets/logo.svg");
        assert!(svg.starts_with("HTTP/1.1 200 OK"), "{svg}");
        assert!(svg.contains("Content-Type: image/svg+xml"), "{svg}");
        let policy = svg
            .lines()
            .find_map(|l| l.strip_prefix("Content-Security-Policy: "))
            .unwrap_or_else(|| panic!("no policy: {svg}"));
        assert!(policy.split(';').any(|p| p.trim() == "sandbox"), "{policy}");
        assert!(policy.contains("script-src 'none'"), "{policy}");
        // The site's own page runs its code.
        let page = ask("/");
        assert!(!page.contains("Content-Security-Policy"), "{page}");
    }

    #[test]
    fn an_ljos_web_without_its_packages_says_to_install_them() {
        let dir = tempfile::tempdir().unwrap();
        let web = dir.path().join("ljos-web");
        fs::create_dir_all(web.join("packages/runtime")).unwrap();
        fs::write(web.join("packages/runtime/package.json"), "{}").unwrap();
        let why = find_ljos_web(dir.path(), Some("ljos-web")).unwrap_err();
        assert!(why.contains("pnpm install"), "{why}");
        let why = find_ljos_web(dir.path(), Some("elsewhere")).unwrap_err();
        assert!(why.contains("elsewhere"), "{why}");
        for package in PACKAGES {
            let at = web.join(MODULES).join(package);
            fs::create_dir_all(&at).unwrap();
            fs::write(at.join("package.json"), "{}").unwrap();
        }
        assert_eq!(
            find_ljos_web(dir.path(), Some("ljos-web")).unwrap(),
            web.canonicalize().unwrap()
        );
    }

    /// An ljos-web in `dir` with its packages installed, its node `node`.
    fn ljos_web(dir: &Path, node: PathBuf) -> Toolchain {
        let web = dir.join("ljos-web");
        fs::create_dir_all(web.join("packages/runtime")).unwrap();
        fs::write(web.join("packages/runtime/package.json"), "{}").unwrap();
        for package in PACKAGES {
            let at = web.join(MODULES).join(package);
            fs::create_dir_all(&at).unwrap();
            fs::write(at.join("package.json"), "{}").unwrap();
        }
        Toolchain {
            ljos_web: web,
            qmlc: dir.join("qmlc"),
            node,
        }
    }

    // A folder of that name that is not one, or one not installed, is
    // passed over for the next place, as a place with nothing there is.
    #[test]
    fn a_place_that_holds_no_ready_ljos_web_is_passed_over() {
        let dir = tempfile::tempdir().unwrap();
        let unready = dir.path().join("first/ljos-web");
        fs::create_dir_all(&unready).unwrap();
        let tools = ljos_web(&dir.path().join("second"), dir.path().join("node"));
        let found = ljos_web_in(vec![
            dir.path().join("nothing"),
            unready.clone(),
            tools.ljos_web.clone(),
        ])
        .unwrap();
        assert_eq!(found, tools.ljos_web.canonicalize().unwrap());
        // With none ready, what the first one lacks is said.
        let why = ljos_web_in(vec![unready.clone()]).unwrap_err();
        assert!(why.contains("packages/runtime"), "{why}");
    }

    // The log of the build before is no account of this one.
    #[test]
    fn a_build_that_fails_before_it_logs_names_no_log() {
        let root = tempfile::tempdir().unwrap();
        let work = root.path().join(SITE_WORK);
        fs::create_dir_all(&work).unwrap();
        fs::write(work.join("build.log"), "the last build's").unwrap();
        let tools = Toolchain {
            ljos_web: root.path().join("none"),
            qmlc: root.path().join("none"),
            node: root.path().join("none"),
        };
        let refused = build(root.path(), "build/web", &tools, false)
            .err()
            .expect("the build was refused");
        let details = refused.details.unwrap();
        assert_eq!(details["kind"], "build_failed");
        assert_eq!(details["log"], serde_json::Value::Null, "{details}");
    }

    // A builder that never ends is stopped, and the next build is not kept
    // waiting on it.
    #[cfg(unix)]
    #[test]
    fn a_build_that_takes_too_long_is_stopped() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let node = root.path().join("node");
        // A child of its own too, which holds the output open.
        fs::write(&node, "#!/bin/sh\nsleep 30 &\nsleep 30\n").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
        let tools = ljos_web(root.path(), node);
        fs::create_dir_all(root.path().join("build/web")).unwrap();
        for _ in 0..2 {
            let (tx, rx) = std::sync::mpsc::channel();
            let (at, tools) = (root.path().to_owned(), tools.clone());
            std::thread::spawn(move || {
                let _ = tx.send(build_within(
                    &at,
                    "build/web",
                    &tools,
                    false,
                    Duration::from_secs(1),
                ));
            });
            let refused = rx
                .recv_timeout(Duration::from_secs(20))
                .expect("the build was stopped")
                .err()
                .expect("the build was refused");
            let details = refused.details.unwrap();
            assert_eq!(details["kind"], "build_failed");
            assert!(
                details["why"].as_str().unwrap().contains("longer than"),
                "{details}"
            );
        }
    }

    // Green before the change too: what it proves is that a file is served
    // whole and its length said, not how much memory serving it takes.
    #[test]
    fn a_large_file_is_served_whole_and_a_head_request_gets_its_length() {
        let (_dir, site) = site();
        let big: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(site.join("assets/big.bin"), &big).unwrap();
        let url = serve(&site).unwrap();
        let port: u16 = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let ask = |method: &str| {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let request =
                format!("{method} /assets/big.bin HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
            stream.write_all(request.as_bytes()).unwrap();
            let mut said = Vec::new();
            stream.read_to_end(&mut said).unwrap();
            let end = said.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            (
                String::from_utf8_lossy(&said[..end]).into_owned(),
                said[end..].to_vec(),
            )
        };
        let (head, body) = ask("HEAD");
        assert!(head.contains("Content-Length: 3000000\r\n"), "{head}");
        assert!(body.is_empty());
        let (head, body) = ask("GET");
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert!(body == big, "{} bytes served", body.len());
    }

    // The site is the directory it was when served: one put in its place
    // later through a link is not read.
    #[cfg(unix)]
    #[test]
    fn a_site_swapped_for_a_link_is_not_read() {
        let (dir, site) = site();
        let url = serve(&site).unwrap();
        let port: u16 = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let other = dir.path().join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("index.html"), "theirs").unwrap();
        fs::rename(&site, dir.path().join("site-before")).unwrap();
        std::os::unix::fs::symlink(&other, &site).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let request = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut said = String::new();
        stream.read_to_string(&mut said).unwrap();
        assert!(said.starts_with("HTTP/1.1 404"), "{said}");
    }
}
