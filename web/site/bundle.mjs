// Reads a site bundle's manifest (`ljos-site.json`) for `mise run
// export:web:build`: holds it to the contract, copies the files it lists,
// and only those, to where the site is built from, and says what the build
// needs besides ljos-web: Qt's QML for the controls, Ljós's for its design.
//
//   node web/site/bundle.mjs BUNDLE DEST LJOS_WEB_PACKAGE_JSON

import { copyFileSync, lstatSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize, sep } from "node:path";

const [bundle, dest, packageJson] = process.argv.slice(2);
const fail = (message) => {
  console.error(`${bundle}: ${message}`);
  process.exit(1);
};

let manifest;
try {
  manifest = JSON.parse(readFileSync(join(bundle, "ljos-site.json"), "utf8"));
} catch (e) {
  fail(`no site bundle: ${e.message}. Export one with the tool \`export\` and \`"target": "web"\``);
}
const { format, version, target, entry, files, imports, modules } = manifest;
if (format !== "ljos-site" || version !== 1 || target !== "web") {
  fail(`a ${format} ${version} for ${target} is not a site bundle this builds (ljos-site 1, web)`);
}
const listed = new Set(files.map((f) => f.path));
if (!listed.has(entry)) fail(`its entry ${entry} is not one of its files`);
for (const m of modules) if (!listed.has(m.qmldir)) fail(`${m.module}'s ${m.qmldir} is not one of its files`);

// What the checkout's runtime package exports: `./QtQuick/Layouts` is the
// module QtQuick.Layouts. The controls are compiled from Qt's own QML and
// Ljos.Design from Ljós's, so they are asked for, not exported.
const exported = new Set(
  Object.keys(JSON.parse(readFileSync(packageJson, "utf8")).exports)
    .filter((k) => k.startsWith("./"))
    .map((k) => k.slice(2).replaceAll("/", ".")),
);
const needs = new Set();
for (const module of imports) {
  if (module === "QtQuick.Controls" || module.startsWith("QtQuick.Controls.")) needs.add("qt");
  else if (module === "Ljos.Design") needs.add("design");
  else if (!exported.has(module)) fail(`it imports ${module}, which this ljos-web does not export`);
}

for (const { path, bytes } of files) {
  const at = normalize(path);
  if (at.startsWith("..") || at.startsWith(sep) || at !== path) fail(`${path} is not a path below the bundle`);
  const from = join(bundle, path);
  const stat = lstatSync(from, { throwIfNoEntry: false });
  if (!stat?.isFile()) fail(`${path} is listed but is not a file of it`);
  if (stat.size !== bytes) fail(`${path} is ${stat.size} bytes; its manifest says ${bytes}`);
  mkdirSync(dirname(join(dest, path)), { recursive: true });
  copyFileSync(from, join(dest, path));
}
copyFileSync(join(bundle, "ljos-site.json"), join(dest, "ljos-site.json"));
console.log([...needs].join(" "));
