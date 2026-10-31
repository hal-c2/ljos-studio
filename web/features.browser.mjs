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
// failure says so: by default a `not_supported` refusal naming the request.
// A scenario that could pass without what it waits on has a `probe`: a
// step run after the step `after`, whose failure is the wait.
const refused = (request) => ({ request, cause: new RegExp(`"kind":"not_supported","ljos_web_request":${request}\\b`) });
// A type ljos-web cannot compile yet is not refused up front: its compile
// error stops the open, or, when it comes after the open, the frame draws
// nothing.
const uncompiled = (request, type) => ({
  request,
  cause: new RegExp(`\`${type}\` is not a type of anything the file imports|^the preview shows: \\[\\]$`),
});
export const WAITING = new Map([
  ["The preview starts in the base state", refused(4)],
  ["A click on the trigger goes to the state", refused(4)],
  ["A click at a point goes to the state", refused(4)],
  ["A click lands at its point, whatever is drawn there", refused(4)],
  // `Ljos.Models` is not in ljos-web's runtime (request 21).
  ["features/preview/json-models.feature: The preview shows a row for each record", uncompiled(21, "JsonListModel")],
  ["A save of the JSON shows without a reload", uncompiled(21, "JsonListModel")],
  ["The mocked state reads the fixture", uncompiled(21, "JsonListModel")],
  // The web preview makes no stand-ins from a project's type dumps
  // (request 22).
  ["A type the project's dump describes stands in", uncompiled(22, "Player")],
  ["An edit to what is given the stand-in shows at once", uncompiled(22, "Player")],
  ["The dumps of a project run as one", uncompiled(22, "Player")],
  ["A dump that does not read stops the preview with the reason", uncompiled(22, "Player")],
  ["A click where no trigger is changes nothing", refused(4)],
  ["A click alone plays the flow and reads nothing", refused(4)],
  ["A picture of the preview", refused(2)],
  ["A mocked block's state is set from outside", refused(4)],
  ["A live block's state is the app's own", refused(4)],
  // ljos-web's `Ljos.Design` is the module's QML; it reads no theme file
  // beside the entry, so a token set is not drawn (request 20). Nothing
  // refuses it: the page cannot tell, so the failure is the drawn text.
  ["A token set shows in the preview and is reported as a reload", { request: 20, cause: /^the preview shows: \["gap 16"\]$/ }],
  // Its last step expects the module's value, which a preview that never
  // drew the token shows too: it would pass having shown nothing. So the
  // token is looked for once it is set (`probe`), as the scenario above
  // does, and its absence is the wait.
  [
    "Undo of a token shows the module's value again",
    {
      request: 20,
      cause: /^the preview shows: \["gap 16"\]$/,
      probe: { after: 'the agent has called the tool "theme_set" with:', then: 'the preview shows "gap 20"' },
    },
  ],
]);

// The steps this runner has and `web/features.mjs` has not: the native
// runner's preview steps, which features.mjs lists as run here.
const PREVIEW_STEPS = [
  {
    kinds: ["then"],
    pattern: /^the preview shows "([^"]*)"$/,
    // DECISION: what the frame draws, as text: an element whose own text
    // is the text, as the native outline names a Text by its text. It is
    // the test's eye, not an outline given to the agent (ljos-web request 3).
    async action(w, s, text) {
      const shown = await w.page.evaluate(
        async ({ text, ms }) => {
          const frame = document.querySelector("iframe");
          const drawn = () =>
            [...frame.contentDocument.querySelectorAll("#preview *")].map((e) =>
              [...e.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE).map((n) => n.textContent).join("").trim(),
            );
          // The mount's effects run after the reload answers; give them
          // until the next frames are drawn.
          for (let i = 0; i < ms / 16 && !drawn().includes(text); i++) await new Promise((r) => requestAnimationFrame(r));
          return drawn().includes(text) ? null : drawn().filter(Boolean);
        },
        { text, ms: 1000 },
      );
      if (shown) throw new Error(`the preview shows: ${JSON.stringify(shown)}`);
    },
  },
  {
    kinds: ["then"],
    pattern: /^the answer has a PNG picture at most (\d+) pixels on its longest side$/,
    action(w, s, side) {
      const png = JSON.stringify(w.ok()).match(/"(iVBORw0KGgo[A-Za-z0-9+/=]+)"/)?.[1];
      if (!png) throw new Error("the answer has no PNG");
      const bytes = Buffer.from(png, "base64");
      const [width, height] = [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
      if (!(width > 0 && height > 0 && Math.max(width, height) <= Number(side))) throw new Error(`the picture is ${width}x${height}`);
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
  await w.ask({ replaceText: [doc, result.generation, start, start + old.length, now] });
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
  const served = await start({ app });
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ ignoreHTTPSErrors: true });
  await context.route(`${ORIGIN}/**`, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname.startsWith("/preview/") || url.pathname === "/compile") {
      const headers = { ...request.headers(), origin: served.url.replace(/\/$/, "") };
      delete headers.host;
      const response = await route.fetch({ url: `${served.url.replace(/\/$/, "")}${url.pathname}${url.search}`, headers });
      return route.fulfill({ response, headers: { ...response.headers(), ...ISOLATED } });
    }
    const body = url.pathname === "/index.html" ? PAGE : readFileSync(join(here, url.pathname));
    return route.fulfill({ status: 200, headers: { "content-type": TYPES[extname(url.pathname)] ?? "application/octet-stream", ...ISOLATED }, body });
  });
  return {
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
      else if (!waits.cause.test(failed.message))
        verdict = `listed as waiting on ljos-web request ${waits.request}, and failed otherwise: ${failed.at}\n      ${failed.message}`;
      if (verdict) {
        counts.failed += 1;
        failures.push(`${scenario.name}: ${verdict}`);
        console.log(`  ✘ ${scenario.name}\n      ${verdict}${said.length ? `\n      ${said.join("\n      ")}` : ""}`);
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
  if (process.env.FEATURES_TIMES) console.log(JSON.stringify(times));
  await chromium.close();
  if (failures.length > 0) process.exitCode = 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main(process.argv.slice(2));
