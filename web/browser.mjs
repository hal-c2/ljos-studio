// Studio for the web in a headless browser: the project kept in OPFS across
// a reload of the page, and a preview asked through the worker, the page
// thread and a frame (`preview.js`, `page.js`), with the numbers DESIGN.md
// S5 reports.
//
//   PLAYWRIGHT=/path/to/node_modules/playwright node web/browser.mjs
//
// Nothing is served from a network: Playwright answers the page's requests
// from this directory, with the headers that make the page cross-origin
// isolated (a shared buffer needs them). The frame mounts with a stand-in
// for a QML compiler, which shows the entry's text as it is: ljos-web has
// none that runs in a page, and the point here is that the files arrive.

import { readFileSync } from "node:fs";
import { dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { document } from "./bench.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const { chromium } = await import(process.env.PLAYWRIGHT ?? "playwright");
const ORIGIN = "https://studio.test";

const TYPES = { ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const ISOLATED = {
  "cross-origin-opener-policy": "same-origin",
  "cross-origin-embedder-policy": "require-corp",
  "cross-origin-resource-policy": "same-origin",
};

const PAGES = {
  "/index.html": `<!doctype html><meta charset="utf-8"><body>
<script type="module">
  import { answerBuffer, relay } from "./preview.js";
  window.worker = (open, frame) => {
    const worker = new Worker("./worker.js", { type: "module" });
    let next = 0;
    const waiting = new Map();
    worker.addEventListener("message", ({ data }) => {
      if (data?.id !== undefined && waiting.has(data.id) && !("ljos-preview" in data)) {
        waiting.get(data.id)(data.result ?? data);
        waiting.delete(data.id);
      }
    });
    const ask = (message) => new Promise((resolve) => {
      const id = ++next;
      waiting.set(id, resolve);
      worker.postMessage({ id, ...message });
    });
    let stop;
    if (frame) {
      open.buffer = answerBuffer();
      stop = relay({ worker, frame, buffer: open.buffer });
    }
    return { open: () => ask({ open }), call: (call, args) => ask({ call, args }), stop };
  };
  window.frameReady = (src) => new Promise((resolve) => {
    const frame = document.createElement("iframe");
    frame.src = src;
    frame.onload = () => resolve(frame);
    document.body.append(frame);
  });
  window.ready = true;
</script>`,
  "/frame.html": `<!doctype html><meta charset="utf-8"><div id="preview"></div>
<script type="module">
  import { servePreview } from "./page.js";
  const compiler = new URLSearchParams(location.search).has("compiler");
  servePreview({
    element: document.getElementById("preview"),
    // Stand-ins: "compiles" by showing the entry's text; logs a line.
    compile: compiler ? async (files, entry) => ({ text: files[entry] }) : undefined,
    mount: (component, element) => {
      element.textContent = component.text;
      console.log("mounted");
      return { dispose() {} };
    },
  });
</script>`,
};

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: true });
await context.route(`${ORIGIN}/**`, (route) => {
  const path = new URL(route.request().url()).pathname;
  const body = PAGES[path] ?? readFileSync(join(here, path));
  return route.fulfill({
    status: 200,
    headers: { "content-type": TYPES[extname(path)] ?? "application/octet-stream", ...ISOLATED },
    body,
  });
});

const failures = [];
const check = (what, ok, detail) => {
  console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failures.push(what);
};

const page = await context.newPage();
page.on("pageerror", (e) => failures.push(`page error: ${e.message}`));
await page.goto(`${ORIGIN}/index.html`);
await page.waitForFunction(() => window.ready);
check("the page is cross-origin isolated", await page.evaluate(() => crossOriginIsolated));

// --- OPFS: writes and their latency, then the project after a reload -----
const RUNS = 50;
const opfs = await page.evaluate(
  async ({ text, runs }) => {
    const { opfsBackend } = await import("./store.js");
    const { openStudio } = await import("./studio.js");
    const root = await navigator.storage.getDirectory();
    await root.removeEntry("bench", { recursive: true }).catch(() => {});
    const backend = await opfsBackend("bench", root);
    await backend.write("Main.qml", text);
    const writes = [];
    const write = backend.write;
    backend.write = async (path, t) => {
      const start = performance.now();
      await write(path, t);
      writes.push(performance.now() - start);
    };
    const studio = await openStudio({ backend });
    const calls = [];
    for (let i = 0; i < runs; i++) {
      const start = performance.now();
      const answer = await studio.call("property_set", { doc: "Main.qml", node: { id: `r${i}` }, name: "width", value: 5000 + i });
      calls.push(performance.now() - start);
      if (answer.error) return { error: answer.error };
    }
    return { writes, calls, place: backend.place };
  },
  { text: document(), runs: RUNS },
);
check("property_set writes through to OPFS", !opfs.error, opfs.error);

await page.reload();
await page.waitForFunction(() => window.ready);
const kept = await page.evaluate(async () => {
  const { opfsBackend } = await import("./store.js");
  return (await (await opfsBackend("bench")).load())["Main.qml"];
});
check("the project is in OPFS after the page reloads", kept?.includes(`width: ${5000 + RUNS - 1}`), kept?.slice(0, 80));

// --- The preview bridge: worker → page thread → frame and back -----------
const bridge = await page.evaluate(async (origin) => {
  const files = { "Main.qml": "import QtQuick\n\nItem {\n    id: root\n    width: 10\n}\n" };
  const out = {};
  for (const src of [`${origin}/frame.html?compiler`, `${origin}/frame.html`]) {
    const frame = await window.frameReady(src);
    const studio = window.worker({ backend: "memory", files, caps: "inspect,mutate,run", timeoutMs: 5000 }, frame);
    await studio.open();
    const start = performance.now();
    const open = await studio.call("preview_open", { doc: "Main.qml", target: "phone" });
    const openMs = performance.now() - start;
    const set = await studio.call("property_set", { doc: "Main.qml", node: { id: "root" }, name: "width", value: 20 });
    const shows = frame.contentDocument.getElementById("preview").textContent;
    const events = await studio.call("preview_events", {});
    const outline = await studio.call("preview_screenshot", { format: "outline" });
    const size = frame.contentDocument.getElementById("preview").style.width;
    studio.stop();
    frame.remove();
    const gone = await studio.call("preview_screenshot", { format: "outline" });
    out[src.endsWith("compiler") ? "compiler" : "plain"] = { open, openMs, set, shows, events, outline, size, gone };
  }
  return out;
}, ORIGIN);

const c = bridge.compiler;
check("preview_open reaches the frame", c.open.ok?.size?.width === 390, c.open);
check("the frame is laid out at the target's size", c.size === "390px", c.size);
check("an edit reaches the frame's files", c.shows.includes("width: 20"), c.shows);
check(
  "the reload and the frame's log line are events",
  c.events.ok?.events?.some((e) => e.kind === "reloaded") && c.events.ok.events.some((e) => e.kind === "log" && e.message === "mounted"),
  c.events,
);
check("what ljos-web cannot answer is refused as unsupported", c.outline.error?.details?.code === "unsupported" || /unsupported/.test(c.outline.error?.message ?? ""), c.outline);
check("a removed frame ends the preview", c.gone.error?.details?.kind === "ended" || c.gone.error?.details?.kind === "no_preview", c.gone);
const p = bridge.plain;
check("without a compiler the preview does not start, and says why", p.open.error?.details?.kind === "start" && /no QML compiler/.test(p.open.error.message), p.open);

const sorted = (xs) => [...xs].sort((a, b) => a - b);
const pct = (xs, q) => +sorted(xs)[Math.min(xs.length - 1, Math.ceil((q / 100) * xs.length) - 1)].toFixed(3);
console.log(
  JSON.stringify(
    {
      browser: browser.version(),
      opfs_write_ms: opfs.writes && { p50: pct(opfs.writes, 50), p95: pct(opfs.writes, 95), n: opfs.writes.length },
      property_set_with_opfs_ms: opfs.calls && { p50: pct(opfs.calls, 50), p95: pct(opfs.calls, 95), n: opfs.calls.length },
      preview_open_over_bridge_ms: +c.openMs.toFixed(3),
    },
    null,
    2,
  ),
);
await browser.close();
if (failures.length > 0) {
  console.log(`\n${failures.length} failed: ${failures.join("; ")}`);
  process.exitCode = 1;
}
