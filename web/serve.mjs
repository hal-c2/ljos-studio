// Serves Studio's page to a person's browser (`mise run web:serve`): the page
// `vite build` made, with the headers cross-origin isolation needs, and Vite's
// dev server under `/preview/` for the preview frame, which compiles what
// `/compile` is given with ljos-web's plugin. ljos-web has no compiler in a
// page (DESIGN.md S5), so a preview needs a server, and this is only ever the
// person's own: it answers on the loopback interface, and only its own page.
//
//   node web/serve.mjs <the assembled app's directory> [port]

import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createServer as createHttp } from "node:http";
import { dirname, extname, join, normalize } from "node:path";
import { pathToFileURL } from "node:url";

const TYPES = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png", ".ttf": "font/ttf", ".woff2": "font/woff2" };
const ISOLATED = {
  "cross-origin-opener-policy": "same-origin",
  "cross-origin-embedder-policy": "require-corp",
  "cross-origin-resource-policy": "same-origin",
};
const HOST = "127.0.0.1";
const LARGEST_PROJECT = 64 << 20;
// Compiles a frame may still be importing from.
const KEPT = 3;

// A file's place in a project: relative, and inside it.
const inside = (path) => !/^[\\/]|\\/.test(path) && !path.split("/").some((part) => part === "" || part === "." || part === "..");

const body = (req) =>
  new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on("data", (chunk) => {
      size += chunk.length;
      if (size > LARGEST_PROJECT) reject(new Error("that project is too large to preview"));
      else chunks.push(chunk);
    });
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });

// The files qmlc is shown for a project previewed with `aliases`
// (`[{module, dir}]`): an aliased module's own files are the alias
// directory's, as `ljos run --alias` imports them. qmlc finds a module by
// every `qmldir` under its root, and of two declaring one module it takes
// either (ljos-web request 17); Qt takes the one at the module's import
// path, so a `qmldir` elsewhere declaring a module that one at its path
// declares too is not shown (a project's `mocks/`).
export function forQmlc(files, aliases = []) {
  const shown = { ...files };
  const direct = (path, dir) => path.startsWith(`${dir}/`) && !path.slice(dir.length + 1).includes("/");
  for (const { module, dir } of aliases) {
    const at = module.replaceAll(".", "/");
    for (const path of Object.keys(shown)) if (direct(path, at)) delete shown[path];
    for (const [path, text] of Object.entries(files)) if (direct(path, dir)) shown[`${at}/${path.slice(dir.length + 1)}`] = text;
  }
  const declared = (path) => /^\s*module\s+([\w.]+)/m.exec(shown[path])?.[1];
  const qmldirs = Object.keys(shown).filter((p) => p === "qmldir" || p.endsWith("/qmldir"));
  const atPath = new Set(qmldirs.filter((p) => declared(p) && p === `${declared(p).replaceAll(".", "/")}/qmldir`).map(declared));
  for (const p of qmldirs) if (atPath.has(declared(p)) && p !== `${declared(p).replaceAll(".", "/")}/qmldir`) delete shown[p];
  // A Ljós block's `# block Name Version File …` line declares its type
  // to Ljós; qmlc reads only Qt's type lines, so it is shown one as well
  // (ljos-web request 19).
  for (const p of Object.keys(shown).filter((p) => p === "qmldir" || p.endsWith("/qmldir"))) {
    const typed = new Set([...shown[p].matchAll(/^\s*(?:singleton\s+)?([A-Z]\w*)\s+\d/gm)].map((m) => m[1]));
    const blocks = [...shown[p].matchAll(/^#\s*block\s+([A-Z]\w*)\s+(\d+(?:\.\d+)?)\s+(\S+\.qml)\b/gm)].filter((m) => !typed.has(m[1]));
    if (blocks.length > 0) shown[p] = `${shown[p].replace(/\n?$/, "\n")}${blocks.map((m) => `${m[1]} ${m[2]} ${m[3]}`).join("\n")}\n`;
  }
  return shown;
}

export async function start({ app, port = 0 }) {
  process.env.VITE_CONFIG_NATIVE_IGNORE_WARNING = "true";
  // Vite and the plugin, from the checkout the app's `node_modules` links to.
  const { createServer: createVite } = await import(pathToFileURL(join(app, "node_modules/vite/dist/node/index.js")).href);
  const projects = join(app, "projects");
  // The app's vite.config.js gives `--root` only to files under this
  // directory, not to any path that has `projects/<n>` in it.
  process.env.STUDIO_PREVIEW_PROJECTS = projects;
  const vite = await createVite({
    configFile: join(app, "vite.config.js"),
    root: app,
    base: "/preview/",
    appType: "mpa",
    logLevel: "warn",
    server: {
      middlewareMode: true,
      hmr: false,
      // Each compile is a new directory: there is nothing to watch for.
      watch: null,
      fs: { allow: [app, process.env.LJOS_WEB, process.env.LJOS_DESIGN, process.env.QT_INSTALL_QML].filter(Boolean) },
    },
  });

  let compiles = 0;
  const compiled = [];
  const compile = async ({ files, entry, aliases = [] }) => {
    if (typeof files !== "object" || !Object.keys(files).every(inside) || !inside(entry)) {
      return { status: 400, answer: { error: "a file's path must be relative, and inside the project" } };
    }
    if (!Array.isArray(aliases) || !aliases.every((a) => /^\w+(\.\w+)*$/.test(a?.module) && inside(a?.dir))) {
      return { status: 400, answer: { error: "an alias is a module name and a directory inside the project" } };
    }
    files = forQmlc(files, aliases);
    const n = ++compiles;
    const dir = join(projects, String(n));
    for (const [path, text] of Object.entries(files)) {
      const file = join(dir, path);
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, text);
    }
    rmSync(join(projects, String(n - KEPT)), { recursive: true, force: true });
    // Asked here, so what the compiler says comes back as it said it, not as
    // the browser's "failed to fetch a module".
    try {
      for (const path of Object.keys(files).filter((p) => p.endsWith(".qml"))) {
        await vite.transformRequest(`/projects/${n}/${path}`);
      }
    } catch (e) {
      const error = String(e?.message ?? e).split(dir + "/").join("");
      compiled.push(error);
      return { status: 200, answer: { error } };
    }
    return { status: 200, answer: { url: `/preview/projects/${n}/${entry}?import` } };
  };

  const dist = join(app, "dist");
  const server = createHttp(async (req, res) => {
    for (const [name, value] of Object.entries(ISOLATED)) res.setHeader(name, value);
    const url = new URL(req.url, `http://${HOST}`);
    // Another site's page may ask a loopback port too, and a name it controls
    // may resolve to it; neither is this page.
    const own = (origin) => origin === `http://${HOST}:${port}` || origin === `http://localhost:${port}`;
    if (!own(`http://${req.headers.host}`) || (req.headers.origin && !own(req.headers.origin))) {
      res.writeHead(403).end("this server answers only its own page");
      return;
    }
    if (url.pathname === "/compile" && req.method === "POST") {
      try {
        const { status, answer } = await compile(JSON.parse(await body(req)));
        res.writeHead(status, { "content-type": "application/json" }).end(JSON.stringify(answer));
      } catch (e) {
        res.writeHead(400, { "content-type": "application/json" }).end(JSON.stringify({ error: String(e?.message ?? e) }));
      }
      return;
    }
    if (url.pathname.startsWith("/preview/")) {
      vite.middlewares(req, res);
      return;
    }
    const path = normalize(join(dist, url.pathname === "/" ? "index.html" : decodeURIComponent(url.pathname)));
    if (!path.startsWith(dist) || !existsSync(path) || !statSync(path).isFile()) {
      res.writeHead(404).end("not found");
      return;
    }
    res.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream" }).end(readFileSync(path));
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, HOST, resolve);
  });
  port = server.address().port;
  return {
    url: `http://${HOST}:${port}/`,
    // Every compile error answered, for a runner to tell why a frame is
    // empty.
    compiled,
    async stop() {
      server.close();
      await vite.close();
    },
  };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [app, port = "5199"] = process.argv.slice(2);
  try {
    const { url } = await start({ app, port: Number(port) });
    console.log(`\nLjós Studio is at ${url}\nProjects stay in that browser's storage. Ctrl-C stops the server.`);
  } catch (e) {
    if (e?.code !== "EADDRINUSE") throw e;
    console.error(`Port ${port} is in use: run PORT=<another> mise run web:serve`);
    process.exit(1);
  }
}
