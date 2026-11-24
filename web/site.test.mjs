// What Studio's site builder (`site/build.mjs`) writes and what it leaves
// alone, with a Vite that only copies the page: these are the builder's own
// rules, and need no ljos-web. Run by `mise run web:serve:test`.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const VITE = `
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
export async function build({ root, build: { outDir } }) {
  if (process.env.STANDIN_FAIL) {
    const summary = "\\x1b[31mBuild failed with 1 error:\\n\\n[plugin ljos-web]\\x1b[39m";
    throw Object.assign(new Error(summary), { errors: [{ message: process.env.STANDIN_FAIL }] });
  }
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(join(outDir, "assets"), { recursive: true });
  copyFileSync(join(root, "index.html"), join(outDir, "index.html"));
  writeFileSync(join(outDir, "assets", (process.env.STANDIN_ASSET ?? "app") + ".js"), "1");
  if (process.env.STANDIN_MEANWHILE) writeFileSync(process.env.STANDIN_MEANWHILE, "mine");
}
`;

function put(at, text) {
  mkdirSync(dirname(at), { recursive: true });
  writeFileSync(at, text);
}

// A project with an export in `build/web`, and a build of it.
function project(t, page = {}, imports = []) {
  const root = mkdtempSync(join(tmpdir(), "ljos-site-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bundle = join(root, "build", "web");
  const contents = { "Main.qml": "Item {}\n" };
  for (const at of [page.icon, page.image]) if (at) contents[at] = "<svg/>\n";
  const files = Object.entries(contents).map(([path, text]) => {
    put(join(bundle, path), text);
    return { path, bytes: Buffer.byteLength(text) };
  });
  const manifest = { format: "ljos-site", version: 1, target: "web", entry: "Main.qml", files, imports, modules: [], page, ljos_web: "0".repeat(40) };
  put(join(bundle, "ljos-site.json"), JSON.stringify(manifest));
  const site = join(root, "build", "web-site");
  const build = (env = {}, node = []) => {
    // Studio unpacks the builder afresh for each build.
    const work = join(root, "work");
    for (const name of ["build.mjs", "index.html"]) {
      mkdirSync(work, { recursive: true });
      copyFileSync(fileURLToPath(new URL(`./site/${name}`, import.meta.url)), join(work, name));
    }
    put(join(work, "node_modules/ljos-web/package.json"), '{"exports":{}}');
    put(join(work, "node_modules/vite/package.json"), '{"type":"module","exports":"./index.js"}');
    put(join(work, "node_modules/vite/index.js"), VITE);
    return spawnSync(process.execPath, [...node, join(work, "build.mjs"), bundle, site], { encoding: "utf8", env: { ...process.env, ...env } });
  };
  return { root, site, build };
}

function built(t, page) {
  const made = project(t, page);
  const first = made.build();
  assert.equal(first.status, 0, first.stdout + first.stderr);
  return made;
}

for (const text of ["Pay $$ less", "R$& co", "Cost $` here", "Yours $' too", "$1 and $<name>"]) {
  test(`the page's head says ${text} as it is written`, (t) => {
    const { site } = built(t, { title: text, description: text });
    const html = readFileSync(join(site, "index.html"), "utf8");
    const said = text.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
    assert.ok(html.includes(`<title>${said}</title>`), html);
    assert.ok(html.includes(`<meta name="description" content="${said}" />`), html);
    assert.equal(html.match(/<!doctype html>/g).length, 1);
  });
}

test("a picture's path keeps its dollars", (t) => {
  const { site } = built(t, { icon: "images/$$.svg" });
  assert.ok(readFileSync(join(site, "index.html"), "utf8").includes('href="./images/$$.svg"'));
});

// A browser asks a page with no icon for `/favicon.ico`, which no site has:
// a 404 in the reader's console on the first load.
test("a page with no icon asks the browser for none", (t) => {
  const { site } = built(t, {});
  const html = readFileSync(join(site, "index.html"), "utf8");
  assert.ok(html.includes('<link rel="icon" href="data:," />'), html);
});

test("a page with an icon names only its own", (t) => {
  const { site } = built(t, { icon: "images/logo.svg" });
  const html = readFileSync(join(site, "index.html"), "utf8");
  assert.deepEqual(html.match(/<link rel="icon"[^>]*>/g), ['<link rel="icon" href="./images/logo.svg" />']);
});

test("a site Studio built is replaced, and nothing is left beside it", (t) => {
  const { root, site, build } = built(t);
  const again = build({ STANDIN_ASSET: "next" });
  assert.equal(again.status, 0, again.stdout + again.stderr);
  assert.deepEqual(readdirSync(join(site, "assets")), ["next.js"]);
  assert.deepEqual(readdirSync(join(root, "build")).sort(), ["web", "web-site"]);
});

const theirs = {
  "a file of a person's": (site) => put(join(site, "mine.txt"), "mine"),
  "a file among the built ones": (site) => put(join(site, "assets/mine.js"), "mine"),
  "an export inside it": (site) => put(join(site, "inner/ljos-site.json"), "{}"),
  "a built file a person edited": (site) => put(join(site, "index.html"), "mine"),
  "an empty directory": (site) => mkdirSync(join(site, "empty")),
  "a link": (site) => symlinkSync("index.html", join(site, "home.html")),
};
for (const [what, add] of Object.entries(theirs)) {
  test(`a site that holds ${what} is not replaced`, (t) => {
    const { root, site, build } = built(t);
    add(site);
    const before = readdirSync(site, { recursive: true }).sort();
    const again = build({ STANDIN_ASSET: "next" });
    assert.equal(again.status, 1, again.stdout);
    assert.match(again.stdout, /^fail .*did not write/m);
    assert.deepEqual(readdirSync(site, { recursive: true }).sort(), before);
    assert.ok(!existsSync(join(site, "assets/next.js")));
    assert.deepEqual(readdirSync(join(root, "build")).sort(), ["web", "web-site"]);
  });
}

test("a file put in the site while it builds is kept", (t) => {
  const { root, site, build } = built(t);
  const again = build({ STANDIN_ASSET: "next", STANDIN_MEANWHILE: join(site, "late.txt") });
  assert.equal(again.status, 1, again.stdout);
  assert.match(again.stdout, /^fail .*late\.txt/m);
  assert.equal(readFileSync(join(site, "late.txt"), "utf8"), "mine");
  assert.deepEqual(readdirSync(join(root, "build")).sort(), ["web", "web-site"]);
});

// The file system refusing a move or a copy, as a full disk or a directory
// the builder may not write does: loaded before the builder, so it sees
// these in place of `node:fs`'s own. A move out of a site set aside is let
// through, so that putting the old site back can be seen.
const REFUSING = `
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
const { REFUSE_FROM, REFUSE_TO, REFUSE_CODE } = process.env;
const refuse = () => {
  throw Object.assign(new Error(\`\${REFUSE_CODE}: refused by the test\`), { code: REFUSE_CODE });
};
const moves = fs.renameSync;
const copies = fs.cpSync;
fs.renameSync = (from, to) => (from === REFUSE_FROM || (to === REFUSE_TO && !String(from).includes(".old-")) ? refuse() : moves(from, to));
fs.cpSync = (from, to, o) => (from === REFUSE_FROM || to === REFUSE_TO ? refuse() : copies(from, to, o));
syncBuiltinESMExports();
`;

function refusing(root, env) {
  const at = join(root, "refusing.mjs");
  writeFileSync(at, REFUSING);
  return [env, ["--import", pathToFileURL(at).href]];
}

test("a site that cannot be written leaves the one before as it was, and says why", (t) => {
  const { root, site, build } = built(t);
  const before = readdirSync(site, { recursive: true }).sort();
  const stamp = readFileSync(join(site, ".ljos-site-build"), "utf8");
  const dist = join(realpathSync(join(root, "work")), "dist");
  const again = build(...refusing(root, { STANDIN_ASSET: "next", REFUSE_FROM: dist, REFUSE_CODE: "ENOSPC" }));
  assert.equal(again.status, 1, again.stdout + again.stderr);
  assert.match(again.stdout, /^fail .*ENOSPC/m);
  assert.deepEqual(readdirSync(site, { recursive: true }).sort(), before);
  assert.equal(readFileSync(join(site, ".ljos-site-build"), "utf8"), stamp);
  assert.deepEqual(readdirSync(join(root, "build")).sort(), ["web", "web-site"]);
});

test("a site that cannot be moved into place puts the one before back, and says why", (t) => {
  const { root, site, build } = built(t);
  const before = readdirSync(site, { recursive: true }).sort();
  const again = build(...refusing(root, { STANDIN_ASSET: "next", REFUSE_TO: site, REFUSE_CODE: "EACCES" }));
  assert.equal(again.status, 1, again.stdout + again.stderr);
  assert.match(again.stdout, /^fail .*EACCES/m);
  assert.deepEqual(readdirSync(site, { recursive: true }).sort(), before);
  assert.ok(!existsSync(join(site, "assets/next.js")));
  assert.deepEqual(readdirSync(join(root, "build")).sort(), ["web", "web-site"]);
});

test("a stamp that does not list the site's files vouches for nothing", (t) => {
  const { site, build } = built(t);
  put(join(site, ".ljos-site-build"), "0".repeat(40));
  assert.equal(build().status, 1);
  assert.ok(existsSync(join(site, "assets/app.js")));
});

// A manifest's paths are logical, `/` between names, on every system: one
// with a `\\` would be a path out of the export where that separates names.
const listing = (t, path) => {
  const made = project(t);
  const bundle = join(made.root, "build", "web");
  const manifest = JSON.parse(readFileSync(join(bundle, "ljos-site.json"), "utf8"));
  if (!path.startsWith("/") && !path.includes("..")) put(join(bundle, path), "1");
  manifest.files.push({ path, bytes: 1 });
  writeFileSync(join(bundle, "ljos-site.json"), JSON.stringify(manifest));
  return made.build();
};

for (const path of ["assets/a.png", "assets/deep/b.png"]) {
  test(`a file below the export listed as ${path} is copied`, (t) => {
    const ran = listing(t, path);
    assert.equal(ran.status, 0, ran.stdout + ran.stderr);
  });
}

for (const path of ["a\\b.png", "assets\\..\\..\\x.png", "../x.png", "/etc/x.png", "a/../b.png", "./a.png", "a//b.png"]) {
  test(`a file listed as ${JSON.stringify(path)} is not a path below the export`, (t) => {
    const ran = listing(t, path);
    assert.equal(ran.status, 1, ran.stdout + ran.stderr);
    assert.match(ran.stdout, /^fail .* is not a path below the export$/m);
  });
}

test("a build that fails says the bundler's first error, uncoloured", (t) => {
  const { build } = project(t);
  const ran = build({ STANDIN_FAIL: "\x1b[31m[plugin ljos-web]\x1b[39m\n\nShell.qml:3: no module Nowhere" });
  assert.equal(ran.status, 1);
  assert.match(ran.stdout, /^fail ljos-web could not build the site: \[plugin ljos-web\] Shell\.qml:3: no module Nowhere$/m);
});

test("Ljos.Design is built from the QML the builder was given", (t) => {
  const { root, build } = project(t, {}, ["Ljos.Design"]);
  const without = build();
  assert.equal(without.status, 1);
  assert.match(without.stdout, /^fail .*Ljos\.Design.* no QML of$/m);
  put(join(root, "work", "standins", "Ljos", "Design", "qmldir"), "module Ljos.Design\n");
  const given = build();
  assert.equal(given.status, 0, given.stdout + given.stderr);
});
