// Studio's scenarios against the wasm build, in node: the same .feature
// files `crates/studio_mcp/tests/features.rs` runs natively, with the same
// steps, on a project held in memory as a browser holds it.
//
//   node web/features.mjs [feature file or directory…]
//
// With no arguments it runs every scenario that needs no preview: a
// preview is a page of its own (`@preview`), a window is native (`@ui`).
//
// DECISION: node, not Playwright. These scenarios touch no DOM and no
// browser storage, and node runs the very module a page loads; the OPFS
// backend and the preview bridge run in a browser in `web/browser.mjs`.
//
// The parser reads the Gherkin these files use (Feature, Rule, Background,
// Scenario, tags, doc strings) and refuses anything else, so a scenario is
// never half read.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { openStudio } from "./studio.js";
import { memoryBackend } from "./store.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const SKIPPED_TAGS = ["preview", "ui"];
const DEFAULT = ["features/tools.feature", "features/edit", "features/project", "features/web"];

// --- Gherkin --------------------------------------------------------------

export function parse(text, file) {
  const lines = text.split("\n");
  const feature = { file, name: "", tags: [], background: [], rules: [], scenarios: [] };
  let tags = [];
  let rule = null; // { name, tags, background, scenarios }
  let steps = null; // where steps go now
  let last = null; // the last step, for its doc string
  const fail = (n, why) => {
    throw new Error(`${file}:${n + 1}: ${why}`);
  };
  for (let n = 0; n < lines.length; n++) {
    const line = lines[n];
    const t = line.trim();
    if (t === "" || t.startsWith("#")) continue;
    if (t.startsWith("@")) {
      tags.push(...t.split(/\s+/).map((tag) => tag.replace(/^@/, "")));
      continue;
    }
    const keyword = /^(Feature|Rule|Background|Scenario|Example):\s*(.*)$/.exec(t);
    if (keyword) {
      const [, kind, name] = keyword;
      if (kind === "Feature") {
        Object.assign(feature, { name, tags });
        steps = null;
      } else if (kind === "Rule") {
        rule = { name, tags, background: [], scenarios: [] };
        feature.rules.push(rule);
        steps = null;
      } else if (kind === "Background") {
        steps = rule ? rule.background : feature.background;
      } else {
        const scenario = { name, tags, line: n + 1, steps: [], rule };
        (rule ? rule.scenarios : feature.scenarios).push(scenario);
        steps = scenario.steps;
      }
      tags = [];
      last = null;
      continue;
    }
    if (/^(Scenario Outline|Scenario Template|Examples|Scenarios):/.test(t) || t.startsWith("|")) {
      fail(n, "tables and outlines are not read by this runner");
    }
    const step = /^(Given|When|Then|And|But|\*) (.*)$/.exec(t);
    if (step) {
      if (!steps) fail(n, "a step outside a scenario or background");
      last = { keyword: step[1], text: step[2], line: n + 1 };
      steps.push(last);
      continue;
    }
    const fence = /^("""|```)(.*)$/.exec(t);
    if (fence) {
      if (!last || last.docstring !== undefined) fail(n, "a doc string that follows no step");
      const body = [];
      // The content type (`"""qml`) is the doc string's first line, as the
      // native runner is given it.
      if (fence[2].trim() !== "") body.push(fence[2].trim());
      for (n++; n < lines.length && lines[n].trim() !== fence[1]; n++) body.push(lines[n]);
      if (n === lines.length) fail(n, "a doc string that does not end");
      last.docstring = body.join("\n");
      continue;
    }
    // A description: free text under a Feature, Rule or Scenario title.
    if (steps && steps.length > 0) fail(n, `not a step: ${t}`);
  }
  return feature;
}

// Every scenario of a feature with its background steps and tags.
export function* scenarios(feature) {
  for (const s of feature.scenarios) yield { ...s, tags: [...feature.tags, ...s.tags], steps: [...feature.background, ...s.steps] };
  for (const rule of feature.rules) {
    for (const s of rule.scenarios) {
      yield {
        ...s,
        tags: [...feature.tags, ...rule.tags, ...s.tags],
        steps: [...feature.background, ...rule.background, ...s.steps],
      };
    }
  }
}

// --- Steps, as `crates/studio_mcp/tests/features.rs` has them -------------

function docstring(step) {
  if (step.docstring === undefined) throw new Error("this step needs a doc string");
  let text = step.docstring;
  const nl = text.indexOf("\n");
  if (nl >= 0 && ["qml", "json", "text"].includes(text.slice(0, nl).trim())) text = text.slice(nl + 1);
  return dedent(text);
}

function dedent(text) {
  const lines = text.split("\n");
  const indent = Math.min(...lines.filter((l) => l.trim() !== "").map((l) => l.length - l.trimStart().length));
  return lines.map((l) => l.slice(Number.isFinite(indent) ? indent : 0)).join("\n");
}

const fileText = (step) => `${docstring(step)}\n`;

function at(value, pointer) {
  let here = value;
  for (const raw of pointer.split("/").slice(1)) {
    const key = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (here === null || typeof here !== "object" || !Object.hasOwn(here, key)) return undefined;
    here = here[key];
  }
  return here;
}

function must(value, pointer) {
  const got = at(value, pointer);
  if (got === undefined) throw new Error(`nothing at \`${pointer}\` in ${JSON.stringify(value)}`);
  return got;
}

function json(text) {
  try {
    return JSON.parse(text);
  } catch (e) {
    throw new Error(`\`${text}\` is not JSON: ${e.message}`);
  }
}

function equal(a, b) {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  return ka.length === kb.length && ka.every((k) => Object.hasOwn(b, k) && equal(a[k], b[k]));
}

class World {
  backend = memoryBackend();
  caps = "inspect,mutate";
  studio = null;
  answer = null; // {ok}|{error}
  kept = new Map();

  constructor(wasm) {
    this.wasm = wasm;
  }

  async server() {
    this.studio ??= await openStudio({ backend: this.backend, caps: this.caps, wasm: this.wasm });
    return this.studio;
  }

  async call(tool, args) {
    const studio = await this.server();
    try {
      this.answer = await studio.call(tool, args);
    } catch (e) {
      // Storage refused what the call wrote: that is the call's answer.
      if (!e.refusal) throw e;
      this.answer = { error: e.refusal };
    }
  }

  ok() {
    if (!this.answer) throw new Error("no tool was called");
    if (this.answer.error) throw new Error(`the tool refused: ${JSON.stringify(this.answer)}`);
    return this.answer.ok;
  }

  refusal() {
    if (!this.answer) throw new Error("no tool was called");
    if (!this.answer.error) throw new Error(`the tool answered: ${JSON.stringify(this.answer.ok)}`);
    return this.answer.error;
  }

  substitute(value) {
    if (typeof value === "string" && value.startsWith("$")) {
      if (!this.kept.has(value.slice(1))) throw new Error(`nothing was kept as \`${value.slice(1)}\``);
      return this.kept.get(value.slice(1));
    }
    if (Array.isArray(value)) return value.map((v) => this.substitute(v));
    if (value && typeof value === "object") {
      return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, this.substitute(v)]));
    }
    return value;
  }
}

const unquote = (t) => t.replaceAll('\\"', '"').replaceAll("\\n", "\n");
const list = (names) =>
  names
    .split(",")
    .map((n) => n.trim())
    .filter((n) => n !== "");

const STEPS = [
  [/^an empty project$/, () => {}],
  [
    /^the file "([^"]+)":$/,
    (w, step, name) => {
      // The native store reads the directory as it is; this one is loaded
      // when the tools connect, so a file given later would not be seen.
      if (w.studio) throw new Error("a file given after the tools connected");
      w.backend.files.set(name, fileText(step));
    },
  ],
  [/^the studio tools are connected$/, async (w) => void (await w.server())],
  [
    /^the studio tools are connected with the capabilities "([^"]*)"$/,
    async (w, step, caps) => {
      w.caps = caps;
      await w.server();
    },
  ],
  [/^the agent (?:has called|calls) the tool "([^"]+)"$/, (w, step, tool) => w.call(tool, {})],
  [
    /^the agent (?:has called|calls) the tool "([^"]+)" with:$/,
    async (w, step, tool) => {
      await w.call(tool, w.substitute(json(docstring(step))));
      // A `Given` sets the scene: a refusal there is the scenario's mistake.
      if (step.kind === "Given") w.ok();
    },
  ],
  [
    /^a person types in the code view of "([^"]+)", replacing "(.*)" with "(.*)"$/,
    async (w, step, doc, old, now) => {
      const studio = await w.server();
      const { text, generation } = studio.text(doc);
      const start = text.indexOf(unquote(old));
      if (start < 0) throw new Error(`\`${unquote(old)}\` is not in ${doc}`);
      await studio.replaceText(doc, generation, start, start + unquote(old).length, unquote(now));
    },
  ],
  [/^the agent keeps the answer's "(\/[^"]*)" as "([^"]+)"$/, (w, step, pointer, name) => void w.kept.set(name, must(w.ok(), pointer))],
  [
    // The value is JSON, which never starts with the `a` of "absent".
    /^the answer's "(\/[^"]*)" is ([^a].*)$/,
    (w, step, pointer, want) => {
      const wanted = w.substitute(json(want));
      const got = must(w.ok(), pointer);
      if (!equal(got, wanted)) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}, not ${JSON.stringify(wanted)}`);
    },
  ],
  [
    /^the answer's "(\/[^"]*)" is:$/,
    (w, step, pointer) => {
      const want = fileText(step);
      const got = must(w.ok(), pointer);
      if (got !== want) throw new Error(`\`${pointer}\` is:\n${got}\nnot:\n${want}`);
    },
  ],
  [
    /^the answer's "(\/[^"]*)" has (\d+) entr(?:y|ies)$/,
    (w, step, pointer, count) => {
      const got = must(w.ok(), pointer);
      if (!Array.isArray(got)) throw new Error(`\`${pointer}\` is not a list: ${JSON.stringify(got)}`);
      if (got.length !== Number(count)) throw new Error(`\`${pointer}\` has ${got.length} entries: ${JSON.stringify(got)}`);
    },
  ],
  [
    /^the answer's "(\/[^"]*)" is absent$/,
    (w, step, pointer) => {
      const got = at(w.ok(), pointer);
      if (got !== undefined) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}`);
    },
  ],
  [
    /^the answer's "(\/[^"]*)" lists "([^"]*)"$/,
    (w, step, pointer, names) => {
      const got = must(w.ok(), pointer);
      if (!equal(got, list(names))) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}`);
    },
  ],
  [
    /^the agent is refused with "([a-z_]+)"$/,
    (w, step, code) => {
      const error = w.refusal();
      if (error.code !== code) throw new Error(`refused with ${JSON.stringify(error)}`);
    },
  ],
  [
    /^the refusal's "(\/[^"]*)" is (.+)$/,
    (w, step, pointer, want) => {
      const wanted = json(want);
      const got = must(w.refusal(), pointer);
      if (!equal(got, wanted)) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}, not ${JSON.stringify(wanted)}`);
    },
  ],
  [
    // What storage holds: a call resolves only once it has the files.
    /^the file "([^"]+)" is:$/,
    (w, step, name) => {
      const want = fileText(step);
      const got = w.backend.files.get(name);
      if (got === undefined) throw new Error(`\`${name}\` does not exist`);
      if (got !== want) throw new Error(`\`${name}\` is:\n${got}\nnot:\n${want}`);
    },
  ],
  [
    /^the file "([^"]+)" does not exist$/,
    (w, step, name) => {
      if (w.backend.files.has(name)) throw new Error(`\`${name}\` exists`);
    },
  ],
  [
    /^storage refuses to keep "([^"]+)"$/,
    (w, step, name) => {
      const write = w.backend.write;
      w.backend.write = async (path, text) => {
        if (path === name) throw new Error("the quota is used up");
        return write(path, text);
      };
      w.restore = () => {
        w.backend.write = write;
      };
    },
  ],
  [/^storage keeps files again$/, (w) => w.restore()],
  [
    /^the agent is offered the tools "([^"]*)"$/,
    async (w, step, names) => {
      const got = (await w.server()).tools().map((t) => t.name);
      if (!equal(got, list(names))) throw new Error(`offered ${JSON.stringify(got)}`);
    },
  ],
];

async function run(step, world) {
  for (const [pattern, action] of STEPS) {
    const m = pattern.exec(step.text);
    if (m) return action(world, step, ...m.slice(1));
  }
  throw new Error("no step matches this");
}

// --- Running --------------------------------------------------------------

function featureFiles(paths) {
  const out = [];
  const walk = (path) => {
    if (statSync(path).isDirectory()) {
      for (const entry of readdirSync(path).sort()) walk(join(path, entry));
    } else if (path.endsWith(".feature")) out.push(path);
  };
  for (const p of paths) walk(resolve(root, p));
  return out;
}

async function main(args) {
  const wasm = readFileSync(join(here, "pkg/studio_web_bg.wasm"));
  const counts = { passed: 0, failed: 0, skipped: 0, steps: 0 };
  const failures = [];
  for (const file of featureFiles(args.length ? args : DEFAULT)) {
    const name = relative(root, file);
    const feature = parse(readFileSync(file, "utf8"), name);
    console.log(`Feature: ${feature.name}  (${name})`);
    for (const scenario of scenarios(feature)) {
      const skip = scenario.tags.find((t) => SKIPPED_TAGS.includes(t));
      if (skip) {
        counts.skipped += 1;
        console.log(`  - ${scenario.name}  (@${skip}: not run here)`);
        continue;
      }
      const world = new World(wasm);
      let kind = "Given";
      let failed = null;
      for (const step of scenario.steps) {
        if (!["And", "But", "*"].includes(step.keyword)) kind = step.keyword;
        try {
          await run({ ...step, kind }, world);
          counts.steps += 1;
        } catch (e) {
          failed = `${name}:${step.line}: ${step.keyword} ${step.text}\n      ${String(e.message).replaceAll("\n", "\n      ")}`;
          break;
        }
      }
      if (failed) {
        counts.failed += 1;
        failures.push(failed);
        console.log(`  ✘ ${scenario.name}\n      ${failed}`);
      } else {
        counts.passed += 1;
        console.log(`  ✔ ${scenario.name}`);
      }
    }
  }
  const total = counts.passed + counts.failed;
  console.log(
    `\n${total} scenarios (${counts.passed} passed, ${counts.failed} failed), ${counts.skipped} not run here, ${counts.steps} steps passed`,
  );
  if (failures.length > 0 || total === 0) process.exitCode = 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main(process.argv.slice(2));
