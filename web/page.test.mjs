// A page whose mount throws, against fakes that copy the two rules of
// ljos-web's that make it matter: `dispose` takes a root out of what can be
// reloaded, and `mount` rethrows what a component throws while being made. No
// QML reaches that throw through a compile, so a scenario cannot. Run by
// `mise run web:serve:test`.

import assert from "node:assert/strict";
import { test } from "node:test";

import { TAG, VERSION } from "./preview.js";
import { servePreview } from "./page.js";

function page({ throwing, inspecting = true }) {
  const mounted = new Set();
  const counts = { mounted: 0, remade: 0, told: 0 };
  const listeners = [];
  const parent = { postMessage: (m) => answers.get(m.id)?.(m.answer) };
  const answers = new Map();
  let id = 0;
  servePreview({
    element: { replaceChildren() {}, style: {} },
    mount(Component) {
      if (throwing()) throw new Error("made threw");
      counts.mounted += 1;
      const root = { object: Component, dispose: () => mounted.delete(root) };
      mounted.add(root);
      return root;
    },
    reload(root) {
      if (!mounted.has(root)) throw new Error("only a root mounted with { reloadable: true } can be reloaded");
      counts.remade += 1;
    },
    inspect: inspecting ? () => ({ outline: () => ({ text: "the root" }) }) : undefined,
    tell: () => (counts.told += 1),
    compile: async () => ({ component: {}, base: "/p/" }),
    window: { console, addEventListener: (_, f) => listeners.push(f), location: { href: "http://page/" } },
    parent,
  });
  const ask = (request) =>
    new Promise((resolve) => {
      id += 1;
      answers.set(id, resolve);
      for (const f of listeners) f({ source: parent, data: { [TAG]: VERSION, id, request } });
    });
  return { ask, counts };
}

const opened = { call: "open", files: {}, entry: "Main.qml", aliases: [], size: { width: 10, height: 10 } };

test("a save after a mount that threw mounts again", async () => {
  let throwing = false;
  const { ask, counts } = page({ throwing: () => throwing });
  await ask(opened);
  throwing = true;
  // With the aliases as they were, so only the cleared root says to mount.
  assert.equal((await ask(opened)).answer, "not_started");
  throwing = false;
  const saved = await ask({ call: "reload", changed: { "Main.qml": "fixed" } });
  assert.equal(saved.outcome, "applied");
  assert.equal(counts.mounted, 2);
  assert.equal(counts.remade, 0);
});

test("a data save after a mount that threw is not told to nothing", async () => {
  let throwing = false;
  const { ask, counts } = page({ throwing: () => throwing });
  await ask(opened);
  throwing = true;
  await ask({ call: "alias", module: "App", dir: "mocks/App", changed: {} });
  throwing = false;
  const saved = await ask({ call: "reload", changed: { "a.json": "{}" } });
  assert.equal(saved.outcome, "applied");
  assert.equal(counts.told, 0);
});

test("the inspector does not answer for a root that is gone", async () => {
  let throwing = false;
  const { ask } = page({ throwing: () => throwing });
  await ask(opened);
  assert.equal((await ask({ call: "outline" })).answer, "outline");
  throwing = true;
  await ask(opened);
  const answer = await ask({ call: "outline" });
  assert.equal(answer.answer, "refused");
  assert.equal(answer.code, "not_running");
});

const inspecting = [
  { call: "screenshot", max_side: 10 },
  { call: "outline" },
  { call: "read", id: "a", names: ["x"] },
  { call: "write", id: "a", name: "x", value: 1 },
  { call: "click", id: "a" },
  { call: "pick", x: 1, y: 1 },
];

test("an app that failed on a switch is not_running to all that ask of it, until a save mends it", async () => {
  let throwing = false;
  const { ask } = page({ throwing: () => throwing });
  await ask(opened);
  throwing = true;
  await ask({ call: "alias", module: "App", dir: "mocks/App", changed: {} });
  for (const request of inspecting) {
    const answer = await ask(request);
    assert.deepEqual([request.call, answer.answer, answer.code], [request.call, "refused", "not_running"]);
    assert.match(answer.message, /failed to start/);
    assert.equal(answer.ljos_web_request, undefined);
  }
  throwing = false;
  assert.equal((await ask({ call: "reload", changed: {} })).outcome, "applied");
  assert.equal((await ask({ call: "outline" })).answer, "outline");
});

test("a running page built without the inspector still waits on ljos-web", async () => {
  const { ask } = page({ throwing: () => false, inspecting: false });
  await ask(opened);
  for (const request of inspecting) {
    const answer = await ask(request);
    assert.deepEqual([request.call, answer.answer], [request.call, "not_supported"]);
    assert.equal(typeof answer.ljos_web_request, "number");
  }
});
