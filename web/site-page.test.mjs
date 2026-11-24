// How a built site's page (`site/main.js`) sizes the entry's root, with the
// entry and ljos-web stood in for: the page's own rules, no browser or build
// needed. Run by `mise run web:serve:test`.

import assert from "node:assert/strict";
import { register } from "node:module";
import { test } from "node:test";

// The page's imports are answered by stand-ins: the root it mounts is the
// one a test gives.
const HOOKS = `
const STANDINS = {
  "ljos-web/runtime.css": "",
  "ljos-web/object": "export const mount = (entry, at) => globalThis.siteRoot(entry, at);",
  "ljos-site:entry": "export default {};",
};
export async function resolve(specifier, context, next) {
  if (specifier in STANDINS) {
    return { url: "data:text/javascript," + encodeURIComponent(STANDINS[specifier]), shortCircuit: true };
  }
  return next(specifier, context);
}
`;
register(`data:text/javascript,${encodeURIComponent(HOOKS)}`);

const MAIN = new URL("./site/main.js", import.meta.url);
let opened = 0;

const rest = async () => {
  for (let i = 0; i < 4; i++) await new Promise((r) => setImmediate(r));
};

// Opens the page in a window `width` by `height` with `root` as the entry's
// root, and waits until it rests.
async function open(root, width, height) {
  const app = { style: {}, querySelector: () => null };
  globalThis.window = { innerWidth: width, innerHeight: height, addEventListener() {} };
  globalThis.document = { getElementById: () => app };
  globalThis.Element = class {};
  globalThis.MutationObserver = class {
    observe() {}
  };
  globalThis.siteRoot = () => ({ object: root });
  await import(`${MAIN}?${++opened}`);
  await rest();
  return app;
}

// A Flickable root whose content, once its picture has loaded, is as tall
// as it is and a little more (a picture bound to `root.height`), saying
// when its content changes.
function growing() {
  const heard = [];
  let height = 0;
  let loaded = false;
  return {
    load() {
      loaded = true;
      for (const f of heard) f();
    },
    width: 0,
    interactive: false,
    contentY: 0,
    get height() {
      return height;
    },
    set height(h) {
      if (h === height) return;
      height = h;
      for (const f of heard) f();
    },
    get contentHeight() {
      return loaded ? height + 100 : 0;
    },
    contentHeightChanged: { connect: (f) => heard.push(f) },
  };
}

test("a page whose content grows with it is the window's height and scrolls inside", async () => {
  const root = growing();
  const app = await open(root, 800, 600);
  root.load();
  await rest();
  assert.deepEqual(
    { width: root.width, height: root.height, interactive: root.interactive, app: app.style.height },
    { width: 800, height: 600, interactive: true, app: "600px" },
  );
});
