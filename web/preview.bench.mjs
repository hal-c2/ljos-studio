// The browser preview's numbers for DESIGN.md S5: `preview_open` and a
// reload over the worker-frame bridge with ljos-web compiling (qmlc behind
// `serve.mjs`), cold (a server that has compiled nothing) and warm, and the
// first preview after a project opens.
//
//   PLAYWRIGHT=… node web/preview.bench.mjs <the assembled app's directory> [runs]

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { BrowserWorld, launch } from "./features.browser.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const [app, runs = "20"] = process.argv.slice(2);
const N = Number(runs);
const sample = readFileSync(join(here, "../samples/hello/Main.qml"), "utf8");

const chromium = await launch(app);
const page = await chromium.page();
const w = new BrowserWorld(page);
w.caps = "inspect,mutate,run";
w.backend.files.set("Main.qml", sample);
// Opening the other document starts another preview, as a person moving
// between documents does; re-opening the open one only resizes it.
w.backend.files.set("Other.qml", sample);

const timed = async (f) => {
  const start = performance.now();
  await f();
  return performance.now() - start;
};
const ok = () => {
  if (w.answer?.error) throw new Error(JSON.stringify(w.answer.error));
};
const call = async (tool, args) => {
  await w.ask({ call: tool, args }).then((a) => {
    w.answer = a.refusal ? { error: a.refusal } : a.result;
  });
  ok();
};

// The project opens, then its first preview: Qt Quick's first compile.
const first = await timed(async () => {
  await w.server();
  await call("preview_open", { doc: "Main.qml", target: "phone" });
});
const reloadCold = await timed(() => call("property_set", { doc: "Main.qml", node: { id: "card" }, name: "width", value: 121 }));
const opens = [];
const reloads = [];
for (let i = 0; i < N; i++) {
  const doc = i % 2 ? "Main.qml" : "Other.qml";
  opens.push(await timed(() => call("preview_open", { doc, target: "phone" })));
  reloads.push(await timed(() => call("property_set", { doc, node: { id: "card" }, name: "width", value: 130 + i })));
}
await chromium.close();

const sorted = (xs) => [...xs].sort((a, b) => a - b);
const pct = (xs, q) => +sorted(xs)[Math.min(xs.length - 1, Math.ceil((q / 100) * xs.length) - 1)].toFixed(1);
console.log(
  JSON.stringify(
    {
      first_preview_after_open_ms: +first.toFixed(1),
      reload_cold_ms: +reloadCold.toFixed(1),
      preview_open_warm_ms: { p50: pct(opens, 50), p95: pct(opens, 95), n: N },
      reload_warm_ms: { p50: pct(reloads, 50), p95: pct(reloads, 95), n: N },
    },
    null,
    2,
  ),
);
