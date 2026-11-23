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

import { createHash } from "node:crypto";
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, normalize, resolve, sep } from "node:path";
import { stripVTControlCharacters } from "node:util";

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
// A site this builder wrote lists what it wrote; anything else there, and
// what was changed since, is somebody's.
const STAMP = ".ljos-site-build";

// What is below `dir` but its stamp: a file by its contents, a directory by
// being one. A link is neither, and is never followed.
function contents(dir, below = "", found = new Map()) {
  for (const e of readdirSync(join(dir, below), { withFileTypes: true })) {
    const path = below + e.name;
    if (path === STAMP && e.isFile()) continue;
    if (e.isDirectory()) {
      found.set(`${path}/`, "directory");
      contents(dir, `${path}/`, found);
    } else if (e.isFile()) {
      found.set(path, createHash("sha256").update(readFileSync(join(dir, path))).digest("hex"));
    } else {
      found.set(path, "other");
    }
  }
  return found;
}

// Why the site at `dir` may not be replaced, if it may not.
function notOurs(dir) {
  const there = lstatSync(dir, { throwIfNoEntry: false });
  if (!there) return null;
  const other = "holds something other than a site Studio built";
  if (!there.isDirectory()) return other;
  const found = contents(dir);
  if (!lstatSync(join(dir, STAMP), { throwIfNoEntry: false })?.isFile()) return found.size ? other : null;
  let wrote;
  try {
    wrote = new Map(Object.entries(JSON.parse(readFileSync(join(dir, STAMP), "utf8")).files));
  } catch {
    return `does not say which files the builder wrote (${STAMP})`;
  }
  const theirs = [...found].filter(([path, what]) => wrote.get(path) !== what).map(([path]) => path);
  if (!theirs.length) return null;
  const more = theirs.length > 3 ? ` and ${theirs.length - 3} more` : "";
  return `holds what the builder did not write, or not as it is now (${theirs.slice(0, 3).join(", ")}${more})`;
}

step("Reading the export");
const keeps = (why) => fail(`${out} ${why}: move that out or choose another place`);
// Asked before the build, which takes a while, and again before the swap.
const kept = notOurs(out);
if (kept) keeps(kept);
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
// QtQuick.Layouts. The controls are QML it compiles, from its own copy of
// Ljós's or from Qt's, so they are not exported. Ljos.Design it has not: the
// QML of that is put in `standins` by whoever runs this.
const runtime = join(here, "node_modules", "ljos-web");
const exported = new Set(
  Object.keys(JSON.parse(readFileSync(join(runtime, "package.json"), "utf8")).exports)
    .filter((k) => k.startsWith("./"))
    .map((k) => k.slice(2).replaceAll("/", ".")),
);
const own = existsSync(join(runtime, "controls"));
for (const module of imports) {
  if (module === "Ljos.Design") {
    if (!existsSync(join(here, "standins", "Ljos", "Design", "qmldir"))) fail(`it imports ${module}, which the builder was given no QML of`);
  } else if (module === "QtQuick.Controls" || module.startsWith("QtQuick.Controls.")) {
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
// With no icon named, a browser asks for `/favicon.ico`, which no site
// has: `data:,` is an icon of nothing, asked of nobody.
head.push(`<link rel="icon" href="${page.icon ? `./${html(page.icon)}` : "data:,"}" />`);
if (page.image) {
  head.push(`<meta property="og:image" content="./${html(page.image)}" />`);
  head.push(`<meta name="twitter:card" content="summary_large_image" />`);
}
const template = readFileSync(join(here, "index.html"), "utf8");
writeFileSync(
  join(here, "index.html"),
  // A function, since a string's `$&` and the like stand for parts of the match.
  template.replace(/<title>[^<]*<\/title>/, () => `<title>${html(title)}</title>\n    ${head.join("\n    ")}`),
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
  // The bundler's own message is a coloured summary that ends at the name
  // of the plugin: what went wrong is in the first of its errors.
  const said = stripVTControlCharacters(String(e.errors?.[0]?.message ?? e.message ?? e))
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
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
const stamp = { ljos_web: manifest.ljos_web, files: Object.fromEntries(contents(dist)) };
writeFileSync(join(dist, STAMP), `${JSON.stringify(stamp, null, 2)}\n`);
mkdirSync(dirname(out), { recursive: true });
// The site before is set aside in a directory made for it: any name chosen
// here could be a directory of the person's.
const aside = there ? mkdtempSync(`${out}.old-`) : null;
if (aside) {
  const before = join(aside, "site");
  renameSync(out, before);
  // Looked at where nobody is writing to it any more, as it is removed there.
  const added = notOurs(before);
  if (added) {
    renameSync(before, out);
    rmdirSync(aside);
    keeps(added);
  }
}
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
