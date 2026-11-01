// Studio's `@preview` scenarios in headless Chromium: the same .feature
// files and steps as `web/features.mjs`, with the session in a worker and
// its preview in a frame that ljos-web compiles and mounts (`page.js`, the
// server Compiler of `serve.mjs`), as a person's browser runs them.
//
//   PLAYWRIGHT=… node web/features.browser.mjs <the assembled app's directory> [feature file or directory…]
//
// Studio's page is answered from this directory, the frame and `/compile`
// by `serve.mjs`, all as one origin (the page is cross-origin isolated, so
// a frame of another would not load). A scenario that needs what ljos-web
// cannot do yet is in WAITING with the request it waits on: it runs, and
// counts as waiting only when it fails for that cause (a `not_supported`
// refusal naming the request), so one ljos-web has made possible says so.

import { readFileSync } from "node:fs";
import { dirname, extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { World, featureFiles, nativeSteps, parse, run, scenarios } from "./features.mjs";
import { start } from "./serve.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const ORIGIN = "https://studio.test";
const TYPES = { ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const ISOLATED = {
  "cross-origin-opener-policy": "same-origin",
  "cross-origin-embedder-policy": "require-corp",
  "cross-origin-resource-policy": "same-origin",
};
// A first compile of Qt Quick is a preview's slowest step.
const ANSWER_MS = 120000;

// Scenario → the ljos-web request (DESIGN.md S5) it waits on, and how its
// failure says so: `cause(message, compiled)` is given the failed step's
// message and what the compiler refused during the scenario. By default the
// cause is a `not_supported` refusal naming the request. A scenario that
// could pass without what it waits on has a `probe`: a step run after the
// step `after`, whose failure is the wait.
const said = (pattern) => (message) => pattern.test(message);
const refused = (request) => ({ request, cause: said(new RegExp(`"kind":"not_supported","ljos_web_request":${request}\\b`)) });
// A type ljos-web cannot compile yet is not refused up front: its compile
// error stops the open, or, when it comes after the open, the reload is
// refused and the frame draws nothing. An empty frame is the wait only when
// the compiler refused the type in this scenario; one that draws nothing
// for another cause fails.
const uncompiled = (request, type) => {
  const missing = `\`${type}\` is not a type of anything the file imports`;
  return {
    request,
    cause: (message, compiled) =>
      message.includes(missing) || (message === "the preview shows: []" && compiled.some((e) => e.includes(missing))),
  };
};
export const WAITING = new Map([
  ["Waiting on the preview says when its app wakes next", refused(25)],
  ["The picture shows the scene", { request: 27, cause: said(/0 pixels of green/) }],
  ["An edit inside the scene reaches the picture", { request: 27, cause: said(/0 pixels of red/) }],
  // The page's outline names the objects, without their boxes or files.
  ["The outline lists every item, with its box and where it is written", { request: 26, cause: said(/`\/outline` is .*without "Rectangle#card 10,20/s) }],
]);

// Every event of the preview's, and the reloads among them, as the native
// runner's World reads them.
async function reloads(w) {
  const studio = await w.server();
  const events = [];
  for (;;) {
    const page = (await studio.call("preview_events", { since: events.length })).ok?.events ?? [];
    if (page.length === 0) break;
    events.push(...page);
  }
  return { events, reported: events.filter((e) => e.kind === "reloaded" || e.kind === "reload_failed") };
}

// DECISION: what the frame draws, as text: an element whose own text is the
// text, as the native outline names a Text by its text. It is the test's eye,
// not an outline given to the agent.
async function previewShows(w, text, ms) {
  // The reloads it was read after, for the step below.
  w.reloadsRead = (await reloads(w)).reported.length;
  const shown = await w.page.evaluate(
    async ({ text, ms }) => {
      const frame = document.querySelector("iframe");
      const drawn = () =>
        [...frame.contentDocument.querySelectorAll("#preview *")].map((e) =>
          [...e.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE).map((n) => n.textContent).join("").trim(),
        );
      const until = performance.now() + ms;
      while (performance.now() < until && !drawn().includes(text)) await new Promise((r) => requestAnimationFrame(r));
      return drawn().includes(text) ? null : drawn().filter(Boolean);
    },
    { text, ms },
  );
  if (shown) throw new Error(`the preview shows: ${JSON.stringify(shown)}`);
}

// The steps this runner has and `web/features.mjs` has not: the native
// runner's preview steps, which features.mjs lists as run here.
const PREVIEW_STEPS = [
  {
    kinds: ["then"],
    pattern: /^the preview shows "([^"]*)"$/,
    // The mount's effects run after the reload answers; give them until the
    // next frames are drawn.
    action: (w, s, text) => previewShows(w, text, 1000),
  },
  {
    kinds: ["then"],
    pattern: /^the preview shows "([^"]*)" within (\d+) seconds$/,
    action: (w, s, text, seconds) => previewShows(w, text, Number(seconds) * 1000),
  },
  {
    kinds: ["then"],
    pattern: /^Studio has reported the preview reloaded since it was read$/,
    async action(w) {
      const { events, reported } = await reloads(w);
      const read = w.reloadsRead ?? 0;
      const last = reported.at(-1);
      if (reported.length > read && last.kind === "reloaded") return;
      throw new Error(
        reported.length > read
          ? `the last reload Studio reported failed: ${JSON.stringify(last)}`
          : `no reload since the preview was read (${read} then, ${reported.length} now); events: ${JSON.stringify(events)}`,
      );
    },
  },
  {
    kinds: ["then"],
    pattern: /^the answer has a PNG picture at most (\d+) pixels on its longest side$/,
    action(w, s, side) {
      const png = (w.ok(), JSON.stringify(w.answer)).match(/"(iVBORw0KGgo[A-Za-z0-9+/=]+)"/)?.[1];
      if (!png) throw new Error("the answer has no PNG");
      const bytes = Buffer.from(png, "base64");
      const [width, height] = [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
      if (!(width > 0 && height > 0 && Math.max(width, height) <= Number(side))) throw new Error(`the picture is ${width}x${height}`);
    },
  },
  {
    kinds: ["then"],
    pattern: /^the answer's picture has (some|no) (red|green)$/,
    async action(w, s, some, colour) {
      const png = (w.ok(), JSON.stringify(w.answer)).match(/"(iVBORw0KGgo[A-Za-z0-9+/=]+)"/)?.[1];
      if (!png) throw new Error("the answer has no PNG");
      const found = await w.page.evaluate(async ([png, at, others]) => {
        const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob());
        const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
        const context = canvas.getContext("2d");
        context.drawImage(bitmap, 0, 0);
        const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);
        let found = 0;
        for (let i = 0; i < data.length; i += 4) {
          if (others.every((o) => data[i + at] > data[i + o] + 24)) found += 1;
        }
        return found;
      }, colour === "red" ? [png, 0, [1, 2]] : [png, 1, [0, 2]]);
      if ((found > 0) !== (some === "some")) throw new Error(`${found} pixels of ${colour}`);
    },
  },
];

const PAGE = `<!doctype html><meta charset="utf-8"><body>
<script type="module">
  import { answerBuffer, relay } from "./preview.js";
  window.studio = async (open) => {
    const frame = document.createElement("iframe");
    frame.src = "/preview/frame.html";
    frame.style.cssText = "width: 1400px; height: 1200px; border: 0";
    // The frame says it is up once its page has loaded (serve/frame.js); one
    // that fails to (a module it cannot import) says nothing, and then the
    // scenario fails on that, not on a preview call's time running out.
    const up = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("the preview frame did not start in 30 s: see the server's log above")), 30000);
      addEventListener("message", function heard({ source, data }) {
        if (source === frame.contentWindow && data?.["ljos-serve-ready"]) {
          clearTimeout(timer);
          removeEventListener("message", heard);
          resolve();
        }
      });
    });
    document.body.append(frame);
    await up;
    const worker = new Worker("./worker.js", { type: "module" });
    let next = 0;
    const waiting = new Map();
    worker.addEventListener("message", ({ data }) => {
      if (data?.id !== undefined && waiting.has(data.id) && !("ljos-preview" in data)) {
        waiting.get(data.id)(data);
        waiting.delete(data.id);
      }
    });
    const ask = (message) => new Promise((resolve) => {
      const id = ++next;
      waiting.set(id, resolve);
      worker.postMessage({ id, ...message });
    });
    open.buffer = answerBuffer();
    const stop = relay({ worker, frame, buffer: open.buffer });
    window.ask = ask;
    window.end = () => { stop(); worker.terminate(); frame.remove(); };
    return ask({ open });
  };
</script>`;

// The World of features.mjs with its session in the page's worker. What
// storage holds is read back after each call, so a step reading
// `backend.files` reads what the worker's storage holds.
export class BrowserWorld extends World {
  constructor(page) {
    super(null);
    this.page = page;
  }

  async ask(message) {
    const answer = await this.page.evaluate((m) => window.ask(m), message);
    if (answer.failure !== undefined && !answer.refusal) throw new Error(answer.failure);
    return answer;
  }

  async server() {
    if (!this.studio) {
      await this.page.evaluate(() => window.end?.());
      const opened = await this.page.evaluate((open) => window.studio(open), {
        backend: "memory",
        files: Object.fromEntries(this.backend.files),
        caps: this.caps,
        timeoutMs: ANSWER_MS,
      });
      if (opened.failure !== undefined) throw new Error(opened.failure);
      const world = this;
      this.studio = {
        stored: (path, text) => world.pending.push(world.ask({ stored: { path, text } })),
        call: async (tool, args) => {
          const answer = await world.ask({ call: tool, args });
          if (answer.refusal) throw Object.assign(new Error(answer.failure), { refusal: answer.refusal });
          return answer.result;
        },
        close: () => world.page.evaluate(() => window.end?.()),
      };
    }
    await Promise.all(this.pending.splice(0));
    return this.studio;
  }

  pending = [];

  async call(tool, args) {
    await super.call(tool, args);
    await this.sync();
  }

  // `backend.files` as the worker's storage holds them after a change.
  async sync() {
    const { result } = await this.ask({ files: true });
    this.backend.files.clear();
    for (const [path, text] of Object.entries(result)) this.backend.files.set(path, text);
  }
}

// The code view step asks the session for a document's text, which is the
// worker's: answered here before the step runs.
async function codeView(w, s) {
  const m = /^a person types in the code view of "([^"]+)", replacing "(.*)" with "(.*)"$/.exec(s.text);
  if (!m || s.kind !== "When") return false;
  const [, doc, old, now] = m.map((t) => t.replaceAll('\\"', '"').replaceAll("\\n", "\n"));
  await w.server();
  const { result } = await w.ask({ text: doc });
  const start = result.text.indexOf(old);
  if (start < 0) throw new Error(`\`${old}\` is not in ${doc}`);
  const typed = await w.ask({ replaceText: [doc, result.generation, start, start + old.length, now] });
  if (typed.refusal) throw Object.assign(new Error(typed.failure), { refusal: typed.refusal });
  await w.sync();
  return true;
}

async function step(s, w) {
  if (await codeView(w, s)) return;
  for (const { kinds, pattern, action } of PREVIEW_STEPS) {
    const m = kinds.includes(s.kind.toLowerCase()) && pattern.exec(s.text);
    if (m) return action(w, s, ...m.slice(1));
  }
  return run(s, w);
}

// Each of this runner's own steps must be the native runner's, so the two
// never mean different things by one sentence.
function drift() {
  const native = new Set(nativeSteps(readFileSync(join(root, "crates/studio_mcp/tests/features.rs"), "utf8")));
  return PREVIEW_STEPS.flatMap((s) => s.kinds.map((k) => `${k} ${s.pattern.source.replaceAll("\\/", "/")}`))
    .filter((s) => !native.has(s))
    .map((s) => `web/features.browser.mjs has \`${s}\`; crates/studio_mcp/tests/features.rs has not`);
}

// Chromium with Studio's page and `serve.mjs` behind one origin.
export async function launch(app) {
  const { chromium } = await import(process.env.PLAYWRIGHT ?? "playwright");
  const served = await start({ app, record: true });
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  // A request that fails is the scenario's evidence, not the run's end: one a
  // page outlives must not take every other scenario down with it, and one a
  // page needs fails that scenario, whose report then names it.
  const dropped = [];
  await context.route(`${ORIGIN}/**`, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    try {
      if (url.pathname.startsWith("/preview/") || url.pathname === "/compile") {
        // A connection of its own: Playwright otherwise reuses one, which the
        // server may close for idling just as the next request goes out.
        const headers = { ...request.headers(), origin: served.url.replace(/\/$/, ""), connection: "close" };
        delete headers.host;
        const response = await route.fetch({ url: `${served.url.replace(/\/$/, "")}${url.pathname}${url.search}`, headers });
        return await route.fulfill({ response, headers: { ...response.headers(), ...ISOLATED } });
      }
      const body = url.pathname === "/index.html" ? PAGE : readFileSync(join(here, url.pathname));
      return await route.fulfill({ status: 200, headers: { "content-type": TYPES[extname(url.pathname)] ?? "application/octet-stream", ...ISOLATED }, body });
    } catch (e) {
      dropped.push(`${request.method()} ${url.pathname}: ${String(e.message).split("\n")[0]}`);
      await route.abort().catch(() => {});
    }
  });
  return {
    // What the compiler refused, in order, since the server started.
    compiled: served.compiled,
    // The requests that got no answer, in order, each with why.
    dropped,
    async page() {
      const page = await context.newPage();
      await page.goto(`${ORIGIN}/index.html`);
      return page;
    },
    async close() {
      await browser.close();
      await served.stop();
    },
  };
}

async function main([app, ...paths]) {
  if (!app) throw new Error("usage: node web/features.browser.mjs <app directory> [feature…]");
  const chromium = await launch(app);
  const failures = drift();
  for (const d of failures) console.log(`✘ steps: ${d}`);
  const counts = { passed: 0, failed: 0, waiting: 0, steps: 0 };
  const seen = new Set();
  const times = [];
  for (const file of featureFiles(paths.length ? paths : ["features/preview"])) {
    const name = relative(root, file);
    const feature = parse(readFileSync(file, "utf8"), name);
    console.log(`Feature: ${feature.name}  (${name})`);
    for (const scenario of scenarios(feature)) {
      if (!scenario.tags.includes("preview")) continue;
      const page = await chromium.page();
      const said = [];
      page.on("pageerror", (e) => said.push(`page error: ${e.message}`));
      const world = new BrowserWorld(page);
      let kind = "Given";
      let failed = null;
      const began = performance.now();
      const compiledFrom = chromium.compiled.length;
      const droppedFrom = chromium.dropped.length;
      // A name two features share is keyed with its file.
      const key = WAITING.has(`${name}: ${scenario.name}`) ? `${name}: ${scenario.name}` : scenario.name;
      const probe = WAITING.get(key)?.probe;
      for (const s of scenario.steps) {
        if (!["And", "But", "*"].includes(s.keyword)) kind = s.keyword;
        try {
          await step({ ...s, kind }, world);
          counts.steps += 1;
        } catch (e) {
          failed = { at: `${name}:${s.line}: ${s.keyword} ${s.text}`, message: String(e.message) };
          break;
        }
        if (probe?.after !== s.text) continue;
        try {
          await step({ keyword: "Then", kind: "Then", text: probe.then, line: s.line }, world);
        } catch (e) {
          failed = { at: `${name}:${s.line}: (WAITING's probe) Then ${probe.then}`, message: String(e.message) };
          break;
        }
      }
      times.push({ scenario: scenario.name, ms: Math.round(performance.now() - began) });
      await page.close();
      const waits = WAITING.get(key);
      if (waits !== undefined) seen.add(key);
      let verdict;
      if (waits === undefined) verdict = failed ? `${failed.at}\n      ${failed.message.replaceAll("\n", "\n      ")}` : null;
      else if (!failed) verdict = `passes: ljos-web request ${waits.request} is answered; take it out of WAITING`;
      else if (!waits.cause(failed.message, chromium.compiled.slice(compiledFrom)))
        verdict = `listed as waiting on ljos-web request ${waits.request}, and failed otherwise: ${failed.at}\n      ${failed.message}`;
      if (verdict) {
        counts.failed += 1;
        failures.push(`${scenario.name}: ${verdict}`);
        const evidence = [...said, ...chromium.dropped.slice(droppedFrom).map((d) => `no answer to ${d}`)];
        console.log(`  ✘ ${scenario.name}\n      ${verdict}${evidence.length ? `\n      ${evidence.join("\n      ")}` : ""}`);
      } else if (waits !== undefined) {
        counts.waiting += 1;
        console.log(`  ⏸ ${scenario.name}  (waits on ljos-web request ${waits.request})`);
      } else {
        counts.passed += 1;
        console.log(`  ✔ ${scenario.name}`);
      }
    }
  }
  for (const name of WAITING.keys()) if (!seen.has(name) && paths.length === 0) failures.push(`WAITING names \`${name}\`, which is no @preview scenario`);
  console.log(`\n${counts.passed + counts.failed + counts.waiting} scenarios (${counts.passed} passed, ${counts.waiting} waiting on ljos-web, ${counts.failed} failed), ${counts.steps} steps passed`);
  for (const d of chromium.dropped) console.log(`no answer to ${d}`);
  if (process.env.FEATURES_TIMES) console.log(JSON.stringify(times));
  await chromium.close();
  if (failures.length > 0) process.exitCode = 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main(process.argv.slice(2));
