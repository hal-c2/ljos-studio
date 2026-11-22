// Studio's site builder (DESIGN.md S6, the site contract): a site bundle
// (`ljos-site.json` and the files it lists) compiled and bundled by
// ljos-web's `qmlc` and Vite plugin into static files. Studio writes this
// file, `index.html`, `main.js` and `vite.config.js` into a work directory
// of its own and runs it there, so a person who installed Studio needs no
// checkout of it; `mise run export:web:build` runs the same files.
//
//   node build.mjs BUNDLE OUT
//
// The work directory's `node_modules` holds ljos-web's packages (Studio
// links them from the ljos-web it found); `QMLC` is the compiler. It
// downloads nothing.
//
// What it does is printed one line at a time for Studio to show: `step
// <what>` as each part starts, `done <bytes>` at the end, and `fail <what
// went wrong, in plain words>` before it stops. Anything else is detail.

import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, normalize, resolve, sep } from "node:path";

const here = import.meta.dirname;
const [bundleArg, outArg] = process.argv.slice(2);
const step = (what) => console.log(`step ${what}`);
const fail = (what, detail) => {
  if (detail) console.error(detail);
  console.log(`fail ${what.replaceAll("\n", " ")}`);
  process.exit(1);
};
if (!bundleArg || !outArg) fail("build.mjs needs the export's directory and where the site goes");
const bundle = resolve(bundleArg);
const out = resolve(outArg);
// A site this builder wrote says so; anything else there is somebody's.
const STAMP = ".ljos-site-build";

step("Reading the export");
let manifest;
try {
  manifest = JSON.parse(readFileSync(join(bundle, "ljos-site.json"), "utf8"));
} catch (e) {
  fail(`${bundle} is not a web export (${e.code === "ENOENT" ? "it has no ljos-site.json" : e.message}): export it for the web first`);
}
const { format, version, target, entry, files, imports, modules, page = {} } = manifest;
if (format !== "ljos-site" || version !== 1 || target !== "web") {
  fail(`a ${format} ${version} for ${target} is not a site bundle this builds (ljos-site 1, web)`);
}
const listed = new Set(files.map((f) => f.path));
if (!listed.has(entry)) fail(`its entry ${entry} is not one of its files`);
for (const m of modules) if (!listed.has(m.qmldir)) fail(`${m.module}'s ${m.qmldir} is not one of its files`);
for (const at of [page.icon, page.image]) if (at && !listed.has(at)) fail(`the page's picture ${at} is not one of its files`);

// What this ljos-web exports: `./QtQuick/Layouts` is the module
// QtQuick.Layouts. The controls and Ljos.Design are QML it compiles, from
// its own copy of Ljós's or from Qt's, so they are not exported.
const runtime = join(here, "node_modules", "ljos-web");
const exported = new Set(
  Object.keys(JSON.parse(readFileSync(join(runtime, "package.json"), "utf8")).exports)
    .filter((k) => k.startsWith("./"))
    .map((k) => k.slice(2).replaceAll("/", ".")),
);
const own = existsSync(join(runtime, "controls"));
for (const module of imports) {
  if (module === "QtQuick.Controls" || module.startsWith("QtQuick.Controls.") || module === "Ljos.Design") {
    if (!own && !process.env.QT_INSTALL_QML) fail(`it imports ${module}, which this ljos-web has no QML of; use a newer ljos-web`);
  } else if (!exported.has(module)) {
    fail(`it imports ${module}, which this ljos-web does not have`);
  }
}

step(`Copying the ${files.length} files the export lists`);
const site = join(here, "site");
rmSync(site, { recursive: true, force: true });
for (const { path, bytes } of files) {
  const at = normalize(path);
  if (at.startsWith("..") || at.startsWith(sep) || at !== path) fail(`${path} is not a path below the export`);
  const from = join(bundle, path);
  const stat = lstatSync(from, { throwIfNoEntry: false });
  if (!stat?.isFile()) fail(`${path} is listed but is not a file of the export: export again`);
  if (stat.size !== bytes) fail(`${path} changed since it was exported: export again`);
  mkdirSync(dirname(join(site, path)), { recursive: true });
  copyFileSync(from, join(site, path));
}
copyFileSync(join(bundle, "ljos-site.json"), join(site, "ljos-site.json"));

// The page's head: what a browser's tab, a search result and a shared link
// show of it.
const html = (text) => String(text).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const head = [];
const title = page.title || entry.replace(/\.qml$/, "");
if (page.description) {
  head.push(`<meta name="description" content="${html(page.description)}" />`);
  head.push(`<meta property="og:description" content="${html(page.description)}" />`);
}
head.push(`<meta property="og:title" content="${html(title)}" />`);
if (page.icon) head.push(`<link rel="icon" href="./${html(page.icon)}" />`);
if (page.image) {
  head.push(`<meta property="og:image" content="./${html(page.image)}" />`);
  head.push(`<meta name="twitter:card" content="summary_large_image" />`);
}
const template = readFileSync(join(here, "index.html"), "utf8");
writeFileSync(
  join(here, "index.html"),
  template.replace(/<title>[^<]*<\/title>/, `<title>${html(title)}</title>\n    ${head.join("\n    ")}`),
);

step("Compiling and bundling with ljos-web");
process.env.VITE_CONFIG_NATIVE_IGNORE_WARNING = "true";
let vite;
try {
  vite = await import("vite");
} catch (e) {
  fail("ljos-web's packages have no Vite: run `pnpm install` in that ljos-web", e.stack);
}
const dist = join(here, "dist");
try {
  await vite.build({ root: here, configFile: join(here, "vite.config.js"), logLevel: "warn", build: { outDir: dist, emptyOutDir: true } });
} catch (e) {
  const said = String(e.message ?? e).trim().split("\n");
  fail(`ljos-web could not build the site: ${said.slice(0, 3).join(" ")}`, e.stack);
}
// The page's pictures are named by the head, which Vite does not read.
for (const at of [page.icon, page.image]) {
  if (!at) continue;
  mkdirSync(dirname(join(dist, at)), { recursive: true });
  copyFileSync(join(site, at), join(dist, at));
}

step(`Writing the site to ${out}`);
const there = lstatSync(out, { throwIfNoEntry: false });
if (there && (!there.isDirectory() || (!existsSync(join(out, STAMP)) && readdirSync(out).length > 0))) {
  fail(`${out} holds something other than a site Studio built; choose another place`);
}
writeFileSync(join(dist, STAMP), `${manifest.ljos_web}\n`);
mkdirSync(dirname(out), { recursive: true });
// The site before is set aside in a directory made for it: any name chosen
// here could be a directory of the person's.
const aside = there ? mkdtempSync(`${out}.old-`) : null;
if (aside) renameSync(out, join(aside, "site"));
try {
  renameSync(dist, out);
} catch {
  // Another file system: copied, then the work directory's is left.
  cpSync(dist, out, { recursive: true });
}
if (aside) rmSync(aside, { recursive: true, force: true });
const weigh = (dir) =>
  readdirSync(dir, { withFileTypes: true }).reduce(
    (sum, e) => sum + (e.isDirectory() ? weigh(join(dir, e.name)) : e.name === STAMP ? 0 : statSync(join(dir, e.name)).size),
    0,
  );
console.log(`done ${weigh(out)}`);
