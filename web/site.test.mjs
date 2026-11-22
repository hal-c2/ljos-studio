// What Studio's site builder (`site/build.mjs`) writes and what it leaves
// alone, with a Vite that only copies the page: these are the builder's own
// rules, and need no ljos-web. Run by `mise run web:serve:test`.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const VITE = `
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
export async function build({ root, build: { outDir } }) {
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
function project(t, page = {}) {
  const root = mkdtempSync(join(tmpdir(), "ljos-site-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bundle = join(root, "build", "web");
  const contents = { "Main.qml": "Item {}\n" };
  for (const at of [page.icon, page.image]) if (at) contents[at] = "<svg/>\n";
  const files = Object.entries(contents).map(([path, text]) => {
    put(join(bundle, path), text);
    return { path, bytes: Buffer.byteLength(text) };
  });
  const manifest = { format: "ljos-site", version: 1, target: "web", entry: "Main.qml", files, imports: [], modules: [], page, ljos_web: "0".repeat(40) };
  put(join(bundle, "ljos-site.json"), JSON.stringify(manifest));
  const site = join(root, "build", "web-site");
  const build = (env = {}) => {
    // Studio unpacks the builder afresh for each build.
    const work = join(root, "work");
    for (const name of ["build.mjs", "index.html"]) {
      mkdirSync(work, { recursive: true });
      copyFileSync(fileURLToPath(new URL(`./site/${name}`, import.meta.url)), join(work, name));
    }
    put(join(work, "node_modules/ljos-web/package.json"), '{"exports":{}}');
    put(join(work, "node_modules/vite/package.json"), '{"type":"module","exports":"./index.js"}');
    put(join(work, "node_modules/vite/index.js"), VITE);
    return spawnSync(process.execPath, [join(work, "build.mjs"), bundle, site], { encoding: "utf8", env: { ...process.env, ...env } });
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

test("a stamp that does not list the site's files vouches for nothing", (t) => {
  const { site, build } = built(t);
  put(join(site, ".ljos-site-build"), "0".repeat(40));
  assert.equal(build().status, 1);
  assert.ok(existsSync(join(site, "assets/app.js")));
});
