// Studio's scenarios against the wasm build, in node: the same .feature
// files `crates/studio_mcp/tests/features.rs` runs natively, with the same
// steps, on a project held in memory as a browser holds it.
//
//   node web/features.mjs [feature file or directory…]
//
// With no arguments it runs every scenario that needs no preview: a
// preview is a page of its own (`@preview`), a window is native (`@ui`),
// and a link, a bundle or a file that is not text is the file system's,
// which storage is not (`@disk`).
//
// A `@preview` scenario runs in Chromium in `web/features.browser.mjs`,
// with these steps and the preview's own.
//
// DECISION: node, not Playwright. These scenarios touch no DOM and no
// browser storage, and node runs the very module a page loads; the OPFS
// backend and the preview bridge run in a browser in `web/browser.mjs`.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { openStudio } from "./studio.js";
import { memoryBackend } from "./store.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
// @clock scenarios move the session's clock on by minutes, which only the
// native runner's timer does (`seconds pass with no call`).
const SKIPPED_TAGS = ["preview", "ui", "disk", "clock"];
const DEFAULT = ["features/tools.feature", "features/theme.feature", "features/edit", "features/project", "features/web"];

// --- Gherkin --------------------------------------------------------------

// The Gherkin these files use: Feature, Rule, Background, Scenario (and
// Scenario Outline with Examples), tags, doc strings and data tables. It
// refuses anything else, so a scenario is never half read. An outline
// becomes one scenario per Examples row, its `<name>`s written in from the
// row in step text, doc strings and tables, as cucumber expands them.

const SCENARIO = /^(Scenario|Example|Scenario Outline|Scenario Template):\s*(.*)$/;
const EXAMPLES = /^(Examples|Scenarios):\s*(.*)$/;

// A table row's cells: `|` separates them, `\|`, `\\` and `\n` are a bar,
// a backslash and a line break, and each cell is trimmed.
function cells(t, fail) {
  if (!t.endsWith("|")) fail("a table row that does not end in `|`");
  const out = [];
  let cell = "";
  for (let i = 1; i < t.length; i++) {
    const c = t[i];
    if (c === "\\" && i + 1 < t.length) {
      const next = t[++i];
      cell += next === "n" ? "\n" : next === "|" || next === "\\" ? next : `\\${next}`;
    } else if (c === "|") {
      out.push(cell.trim());
      cell = "";
    } else cell += c;
  }
  return out;
}

export function parse(text, file) {
  const lines = text.split("\n");
  const feature = { file, name: "", tags: [], background: [], rules: [], scenarios: [] };
  let tags = [];
  let rule = null; // { name, tags, background, scenarios }
  let steps = null; // where steps go now
  let last = null; // the last step, for its doc string or table
  let scenario = null; // the scenario steps go to, for its Examples
  let examples = null; // the Examples block whose rows come now
  const fail = (n, why) => {
    throw new Error(`${file}:${n + 1}: ${why}`);
  };
  const table = (n, t) => {
    const row = cells(t, (why) => fail(n, why));
    let rows;
    if (examples) rows = examples.rows;
    else if (last && last.docstring === undefined) rows = last.table ??= [];
    else fail(n, "a table that follows no step and is no Examples");
    if (rows.length > 0 && row.length !== rows[0].cells.length) {
      fail(n, `a row of ${row.length} cells in a table of ${rows[0].cells.length}`);
    }
    rows.push({ cells: row, line: n + 1 });
  };
  const ended = (n) => {
    // An outline is read whole: every Examples has its header.
    if (scenario?.examples?.some((e) => e.rows.length === 0)) fail(n, "Examples with no table");
    if (scenario?.outline && scenario.examples.length === 0) fail(scenario.line - 1, "an outline with no Examples");
  };
  for (let n = 0; n < lines.length; n++) {
    const t = lines[n].trim();
    if (t === "" || t.startsWith("#")) continue;
    if (t.startsWith("@")) {
      tags.push(...t.split(/\s+/).map((tag) => tag.replace(/^@/, "")));
      continue;
    }
    if (t.startsWith("|")) {
      table(n, t);
      continue;
    }
    const keyword = /^(Feature|Rule|Background):\s*(.*)$/.exec(t);
    const titled = keyword ?? SCENARIO.exec(t);
    if (titled) {
      ended(n);
      const [, kind, name] = titled;
      examples = null;
      if (kind === "Feature") {
        Object.assign(feature, { name, tags });
        steps = null;
        scenario = null;
      } else if (kind === "Rule") {
        rule = { name, tags, background: [], scenarios: [] };
        feature.rules.push(rule);
        steps = null;
        scenario = null;
      } else if (kind === "Background") {
        steps = rule ? rule.background : feature.background;
        scenario = null;
      } else {
        scenario = { name, tags, line: n + 1, steps: [], rule, examples: [], outline: kind !== "Scenario" && kind !== "Example" };
        (rule ? rule.scenarios : feature.scenarios).push(scenario);
        steps = scenario.steps;
      }
      tags = [];
      last = null;
      continue;
    }
    const block = EXAMPLES.exec(t);
    if (block) {
      if (!scenario) fail(n, "Examples under no scenario");
      if (examples?.rows.length === 0) fail(n, "Examples with no table");
      examples = { name: block[2], tags, rows: [] };
      scenario.examples.push(examples);
      tags = [];
      steps = null;
      last = null;
      continue;
    }
    if (tags.length > 0) fail(n, "tags that tag no Feature, Rule, Scenario or Examples");
    const step = /^(Given|When|Then|And|But|\*) (.*)$/.exec(t);
    if (step) {
      if (!steps) fail(n, examples ? "a step among Examples" : "a step outside a scenario or background");
      last = { keyword: step[1], text: step[2], line: n + 1 };
      steps.push(last);
      continue;
    }
    const fence = /^("""|```)(.*)$/.exec(t);
    if (fence) {
      if (!last || last.docstring !== undefined || last.table) fail(n, "a doc string that follows no step");
      const body = [];
      // The content type (`"""qml`) is the doc string's first line, as the
      // native runner is given it.
      if (fence[2].trim() !== "") body.push(fence[2].trim());
      for (n++; n < lines.length && lines[n].trim() !== fence[1]; n++) body.push(lines[n]);
      if (n === lines.length) fail(n, "a doc string that does not end");
      last.docstring = body.join("\n");
      continue;
    }
    // A description: free text under a title, before its steps or rows.
    if ((steps && steps.length > 0) || (examples && examples.rows.length > 0)) fail(n, `not a step: ${t}`);
  }
  ended(lines.length);
  return feature;
}

// `<name>`s in `text` from an Examples row; a name the row lacks stays.
const fill = (text, values) => text.replace(/<([^<>]+)>/g, (whole, name) => (Object.hasOwn(values, name) ? values[name] : whole));

function expand(s, tags, background) {
  const base = { ...s, tags: [...tags, ...s.tags], steps: [...background, ...s.steps] };
  if (s.examples.length === 0) return [base];
  const out = [];
  for (const examples of s.examples) {
    const [header, ...rows] = examples.rows;
    for (const row of rows) {
      const values = Object.fromEntries(header.cells.map((name, i) => [name, row.cells[i]]));
      const filled = (step) => ({
        ...step,
        text: fill(step.text, values),
        ...(step.docstring !== undefined && { docstring: fill(step.docstring, values) }),
        ...(step.table && { table: step.table.map((r) => ({ ...r, cells: r.cells.map((c) => fill(c, values)) })) }),
      });
      out.push({
        ...base,
        name: fill(s.name, values),
        line: row.line,
        tags: [...base.tags, ...examples.tags],
        // The background is not the outline's: its `<name>`s stay.
        steps: [...background, ...s.steps.map(filled)],
      });
    }
  }
  return out;
}

// Every scenario of a feature with its background steps and tags, an
// outline once for each of its Examples rows.
export function* scenarios(feature) {
  for (const s of feature.scenarios) yield* expand(s, feature.tags, feature.background);
  for (const rule of feature.rules) {
    for (const s of rule.scenarios) yield* expand(s, [...feature.tags, ...rule.tags], [...feature.background, ...rule.background]);
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

export class World {
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

  // `client` is the name a second agent gave itself; the scenario's own
  // agent gave none.
  async call(tool, args, client = undefined) {
    const studio = await this.server();
    try {
      this.answer = await studio.call(tool, args, client);
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

// Each step has the kinds a keyword gives it (`Given`, `When`, `Then`; an
// `And` or `But` takes the one before), as cucumber matches a step to the
// functions of its kind only.
const step = (kinds, pattern, action) => ({ kinds: kinds.split(" "), pattern, action });

const calls = (w, s, tool) => w.call(tool, {});
async function callsWith(w, s, tool) {
  await w.call(tool, w.substitute(json(docstring(s))));
  // A `Given` sets the scene: a refusal there is the scenario's mistake.
  if (s.kind === "Given") w.ok();
}
async function secondCallsWith(w, s, name, tool) {
  await w.call(tool, w.substitute(json(docstring(s))), name);
  if (s.kind === "Given") w.ok();
}
const keeps = (w, s, pointer, name) => void w.kept.set(name, must(w.ok(), pointer));

const STEPS = [
  step("given", /^an empty project$/, () => {}),
  ...[["given", /^the file "([^"]+)":$/], ["when", /^somebody writes the file "([^"]+)":$/]].map(([kind, pattern]) =>
    step(kind, pattern, (w, s, name) => {
      // The native store reads the directory as it is; this one holds what
      // it loaded when the tools connected, so a file given later is in
      // storage and told to the tools, as another tab's would be.
      w.backend.files.set(name, fileText(s));
      w.studio?.stored(name, fileText(s));
    }),
  ),
  step("given", /^the studio tools are connected$/, async (w) => void (await w.server())),
  step("given", /^the studio tools are connected with the capabilities "([^"]*)"$/, async (w, s, caps) => {
    // The native runner gives the capabilities with each call; a studio
    // here is opened with them, so one with others is opened again over
    // the same storage.
    if (w.caps !== caps) w.studio = undefined;
    w.caps = caps;
    await w.server();
  }),
  step("given", /^the agent has called the tool "([^"]+)"$/, calls),
  step("when", /^the agent calls the tool "([^"]+)"$/, calls),
  step("given", /^the agent has called the tool "([^"]+)" with:$/, callsWith),
  step("when", /^the agent calls the tool "([^"]+)" with:$/, callsWith),
  step("given", /^a second agent named "([^"]+)" has called the tool "([^"]+)" with:$/, secondCallsWith),
  step("when", /^a second agent named "([^"]+)" calls the tool "([^"]+)" with:$/, secondCallsWith),
  step("when", /^a person types in the code view of "([^"]+)", replacing "(.*)" with "(.*)"$/, async (w, s, doc, old, now) => {
    const studio = await w.server();
    const { text, generation } = studio.text(doc);
    const start = text.indexOf(unquote(old));
    if (start < 0) throw new Error(`\`${unquote(old)}\` is not in ${doc}`);
    await studio.replaceText(doc, generation, start, start + unquote(old).length, unquote(now));
  }),
  step("given when then", /^the agent keeps the answer's "(\/[^"]*)" as "([^"]+)"$/, keeps),
  // The value is JSON, which never starts with the `a` of "absent".
  step("then", /^the answer's "(\/[^"]*)" is ([^a].*)$/, (w, s, pointer, want) => {
    const wanted = w.substitute(json(want));
    const got = must(w.ok(), pointer);
    if (!equal(got, wanted)) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}, not ${JSON.stringify(wanted)}`);
  }),
  step("then", /^the answer's "(\/[^"]*)" is:$/, (w, s, pointer) => {
    const want = fileText(s);
    const got = must(w.ok(), pointer);
    // A string is compared as text; anything else as the JSON the doc
    // string holds, so a list of records reads as one.
    if (typeof got === "string" ? got === want : equal(got, json(want))) return;
    const shown = typeof got === "string" ? got : JSON.stringify(got);
    throw new Error(`\`${pointer}\` is:\n${shown}\nnot:\n${want}`);
  }),
  step("then", /^the answer's "(\/[^"]*)" has (\d+) entr(?:y|ies)$/, (w, s, pointer, count) => {
    const got = must(w.ok(), pointer);
    if (!Array.isArray(got)) throw new Error(`\`${pointer}\` is not a list: ${JSON.stringify(got)}`);
    if (got.length !== Number(count)) throw new Error(`\`${pointer}\` has ${got.length} entries: ${JSON.stringify(got)}`);
  }),
  // One entry of a list, wherever it is in the list: what a scenario says
  // of one member, not of the order of the rest.
  step("then", /^the answer's "(\/[^"]*)" has the entry:$/, (w, s, pointer) => {
    const got = must(w.ok(), pointer);
    if (!Array.isArray(got)) throw new Error(`\`${pointer}\` is not a list: ${JSON.stringify(got)}`);
    const want = json(docstring(s));
    if (!got.some((entry) => equal(entry, want)))
      throw new Error(`\`${pointer}\` has no entry ${JSON.stringify(want)}: ${JSON.stringify(got)}`);
  }),
  step("then", /^the answer's "(\/[^"]*)" is absent$/, (w, s, pointer) => {
    const got = at(w.ok(), pointer);
    if (got !== undefined) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}`);
  }),
  step("then", /^the answer's "(\/[^"]*)" contains "([^"]*)"$/, (w, s, pointer, text) => {
    const got = must(w.ok(), pointer);
    if (typeof got !== "string" || !got.includes(text)) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}, without ${JSON.stringify(text)}`);
  }),
  step("then", /^the answer's "(\/[^"]*)" lists "([^"]*)"$/, (w, s, pointer, names) => {
    const got = must(w.ok(), pointer);
    if (!equal(got, list(names))) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}`);
  }),
  step("then", /^the agent is refused with "([a-z_]+)"$/, (w, s, code) => {
    const error = w.refusal();
    if (error.code !== code) throw new Error(`refused with ${JSON.stringify(error)}`);
  }),
  step("then", /^the refusal's "(\/[^"]*)" is (.+)$/, (w, s, pointer, want) => {
    const wanted = json(want);
    const got = must(w.refusal(), pointer);
    if (!equal(got, wanted)) throw new Error(`\`${pointer}\` is ${JSON.stringify(got)}, not ${JSON.stringify(wanted)}`);
  }),
  // The pattern is a Rust `regex` one; the ones the scenarios use mean the
  // same to JavaScript's.
  step("then", /^the refusal's message matches "(.+)"$/, (w, s, pattern) => {
    const message = w.refusal().message ?? "";
    if (!new RegExp(pattern).test(message)) throw new Error(`the message is ${JSON.stringify(message)}`);
  }),
  // What storage holds: a call resolves only once it has the files.
  step("then", /^the file "([^"]+)" is:$/, (w, s, name) => {
    const want = fileText(s);
    const got = w.backend.files.get(name);
    if (got === undefined) throw new Error(`\`${name}\` does not exist`);
    if (got !== want) throw new Error(`\`${name}\` is:\n${got}\nnot:\n${want}`);
  }),
  step("then", /^the file "([^"]+)" contains "(.*)"$/, (w, s, name, want) => {
    const got = w.backend.files.get(name);
    if (got === undefined) throw new Error(`\`${name}\` does not exist`);
    if (!got.includes(want.replaceAll('\\"', '"'))) throw new Error(`\`${name}\` does not contain ${want}:\n${got}`);
  }),
  step("then", /^the file "([^"]+)" does not contain "(.*)"$/, (w, s, name, want) => {
    const got = w.backend.files.get(name) ?? "";
    if (got.includes(want.replaceAll('\\"', '"'))) throw new Error(`\`${name}\` contains ${want}:\n${got}`);
  }),
  step("then", /^the history has (\d+) entr(?:y|ies)$/, async (w, s, want) => {
    await w.call("history", { action: "list" });
    const got = (w.answer.ok?.done ?? []).length;
    if (got !== Number(want)) throw new Error(`the history has ${got} entries, not ${want}`);
  }),
  step("then", /^the file "([^"]+)" exists$/, (w, s, name) => {
    if (!w.backend.files.has(name)) throw new Error(`\`${name}\` does not exist`);
  }),
  step("given", /^the directory "([^"]+)" is deleted$/, (w, s, name) => {
    const below = `${name.replace(/\/$/, "")}/`;
    const gone = [...w.backend.files.keys()].filter((f) => f.startsWith(below));
    if (gone.length === 0) throw new Error(`\`${name}\`: there is no such directory`);
    // Told to the tools, as another tab's removal would be (see "the file").
    for (const f of gone) {
      w.backend.files.delete(f);
      w.studio?.stored(f, null);
    }
  }),
  // A new studio over the same storage: what a session kept in memory is
  // gone, what it wrote is read again.
  step("given", /^the studio tools are reconnected$/, async (w) => {
    w.studio = undefined;
    await w.server();
  }),
  step("then", /^the file "([^"]+)" does not exist$/, (w, s, name) => {
    if (w.backend.files.has(name)) throw new Error(`\`${name}\` exists`);
  }),
  // Another program's change: here another tab's, told to the tools as
  // "the file" is.
  step("when", /^the file "([^"]+)" is rewritten outside Studio:$/, (w, s, name) => {
    w.backend.files.set(name, fileText(s));
    w.studio?.stored(name, fileText(s));
  }),
  step("when", /^the file "([^"]+)" is deleted outside Studio$/, (w, s, name) => {
    if (!w.backend.files.delete(name)) throw new Error(`\`${name}\` does not exist`);
    w.studio?.stored(name, null);
  }),
  step("then", /^the agent is offered the tools "([^"]*)"$/, async (w, s, names) => {
    const got = (await w.server()).tools().map((t) => t.name);
    if (!equal(got, list(names))) throw new Error(`offered ${JSON.stringify(got)}`);
  }),
  step("then", /^the list of tools is at most (\d+) bytes$/, async (w, s, cap) => {
    const bytes = JSON.stringify({ tools: (await w.server()).tools() }).length;
    if (bytes > Number(cap)) throw new Error(`the list is ${bytes} bytes`);
  }),
  // Browser storage's own (`@web`): the native store is a directory.
  step("given", /^storage refuses to keep "([^"]+)"$/, (w, s, name) => {
    const write = w.backend.write;
    w.backend.write = async (path, text) => {
      if (path === name) throw new Error("the quota is used up");
      return write(path, text);
    };
    w.restore = () => {
      w.backend.write = write;
    };
  }),
  step("when", /^storage keeps files again$/, (w) => w.restore()),
];

// The native runner's steps this one leaves out, and why; and its own that
// the native runner has not. `stepsDrift` holds both lists to the native
// runner's source, so a step added there is a failure here until it is
// written here or listed.
const NATIVE_ONLY = new Map([
  ["when ^the agent asks the server to wait with:$", "a browser preview cannot be waited on yet (ljos-web request 25)"],
  ['then ^the tool "([^"]+)" answers while a wait past the answer\'s rests is parked, and the wait ends with the server$', "a page runs one call at a time, and a browser preview cannot be waited on yet (request 25)"],
  ['when ^the agent opens the project "([^"]+)" by its absolute path$', "a browser's storage has no absolute paths"],
  ["then ^the preview has opened no socket$", "a preview is a page of its own (@preview)"],
  ['then ^the preview shows "([^"]*)"$', "a preview is a page of its own (@preview): web/features.browser.mjs"],
  ['then ^Studio has reported the preview reloaded since it was read$', "a preview is a page of its own (@preview)"],
  ['then ^the preview shows "([^"]*)" within (\\d+) seconds$', "a preview is a page of its own (@preview)"],
  ["then ^the answer has a PNG picture at most (\\d+) pixels on its longest side$", "a picture is a preview's (@preview): web/features.browser.mjs"],
  ['given ^the link "([^"]+)" leads to "([^"]+)"$', "storage holds no links (@disk)"],
  ['given ^the named pipe "([^"]+)"$', "storage holds no pipes (@disk)"],
  ['given ^the link "([^"]+)" leads to a directory outside the project holding "([^"]+)"$', "storage holds no links (@disk)"],
  ['given ^the link "([^"]+)" leads to the file "([^"]+)" outside the project$', "storage holds no links (@disk)"],
  ["then ^nothing was written outside the project$", "storage has no outside (@disk)"],
  ['given ^the file "([^"]+)" holds bytes that are not UTF-8$', "storage holds text (@disk)"],
  ['given ^Studio serves agents with the capabilities "([^"]*)"$', "the endpoint is a native window's (endpoint.feature)"],
  ['when ^a client calls the tool "([^"]+)" on the endpoint( with the token| with another token)?(?: and the (Origin|Host) "([^"]+)")?$', "the endpoint is a native window's (endpoint.feature)"],
  ["then ^the endpoint answers it$", "the endpoint is a native window's (endpoint.feature)"],
  ['then ^the endpoint refuses it with (\\d+) "([a-z_]+)"$', "the endpoint is a native window's (endpoint.feature)"],
  ['then ^the endpoint\'s tool refuses it with "([a-z_]+)"$', "the endpoint is a native window's (endpoint.feature)"],
  ["then ^the endpoint listens on loopback only$", "the endpoint is a native window's (endpoint.feature)"],
  ["then ^the token file can be read by its owner alone$", "the endpoint is a native window's (endpoint.feature)"],
  ["given ^the directory for the endpoint's token can be entered by others$", "the endpoint is a native window's (endpoint.feature)"],
  ['when ^Studio starts serving agents with the capabilities "([^"]*)"$', "the endpoint is a native window's (endpoint.feature)"],
  ['then ^Studio cannot serve agents, saying "([^"]+)"$', "the endpoint is a native window's (endpoint.feature)"],
  ["then ^no token file is written$", "the endpoint is a native window's (endpoint.feature)"],
  ['given ^"([^"]+)" links to a directory outside the project that holds the file "([^"]+)"$', "storage holds no links (@disk)"],
  ['given ^"([^"]+)" links to "([^"]+)" in the project$', "storage holds no links (@disk)"],
  ['given ^a diamond of directory links (\\d+) levels deep in "([^"]+)"( holding no files)?$', "storage holds no links (@disk)"],
  ["when ^(\\d+) seconds pass with no call$", "the session's clock is moved on by the native runner's timer (@clock)"],
  ['when ^"([^"]+)" calls the tool "([^"]+)" on "([^"]+)" every (\\d+) seconds for (\\d+) seconds$', "the session's clock is moved on by the native runner's timer (@clock)"],
  ["then ^it answered within (\\d+) seconds?$", "a call's time is measured on the native side, where the disk is"],
  ['then ^the bundle "([^"]+)", copied out of the project, runs headless and shows what the preview shows$', "a bundle runs as a native process (@preview)"],
  ['then ^the directory outside the project holds only "([^"]*)"$', "storage holds no links (@disk)"],
  ['then ^the answer\'s "(/[^"]*)" names the ljos-web commit Studio is pinned to$', "a bundle is written to disk (@disk)"],
  ['then ^a site builder held to the contract builds "([^"]+)", starting at "([^"]+)"$', "a bundle is written to disk (@disk)"],
]);
const WEB_ONLY = new Set(['given ^storage refuses to keep "([^"]+)"$', "when ^storage keeps files again$"]);

const NATIVE = "crates/studio_mcp/tests/features.rs";

// The steps `crates/studio_mcp/tests/features.rs` declares, as `kind
// pattern`: from its `#[given(…)]`, `#[when(…)]` and `#[then(…)]`,
// which rustfmt breaks over lines when the pattern is long.
export function nativeSteps(source) {
  const out = [];
  for (const m of source.matchAll(/#\[(given|when|then)\(\s*(regex\s*=\s*)?r?(#*)"(.*?)"\3\s*\)\]/g)) {
    const [, kind, regex, , text] = m;
    // A plain string is a cucumber expression; one without parameters
    // matches its text whole.
    if (!regex && /[{}()/\\]/.test(text)) throw new Error(`${NATIVE}: \`${text}\` is an expression this check does not read`);
    out.push(`${kind} ${regex ? text : `^${text.replace(/[.*+?^$|[\]]/g, "\\$&")}$`}`);
  }
  return out;
}

// What differs between the two step libraries, as lines to show.
export function stepsDrift(native) {
  const here = new Set(STEPS.flatMap((s) => s.kinds.map((k) => `${k} ${s.pattern.source.replaceAll("\\/", "/")}`)));
  const there = new Set(native);
  const drift = [];
  if (there.size === 0) drift.push(`no steps were read from ${NATIVE}`);
  for (const s of there) if (!here.has(s) && !NATIVE_ONLY.has(s)) drift.push(`${NATIVE} has \`${s}\`; web/features.mjs has not`);
  for (const s of here) if (!there.has(s) && !WEB_ONLY.has(s)) drift.push(`web/features.mjs has \`${s}\`; ${NATIVE} has not`);
  for (const s of NATIVE_ONLY.keys()) if (!there.has(s)) drift.push(`\`${s}\` is listed as native only, and ${NATIVE} has not got it`);
  for (const s of WEB_ONLY) if (!here.has(s)) drift.push(`\`${s}\` is listed as web only, and web/features.mjs has not got it`);
  return drift;
}

export async function run(s, world) {
  if (s.table) throw new Error("this runner's steps take no table");
  for (const { kinds, pattern, action } of STEPS) {
    if (!kinds.includes(s.kind.toLowerCase())) continue;
    const m = pattern.exec(s.text);
    if (m) return action(world, s, ...m.slice(1));
  }
  throw new Error(`no ${s.kind} step matches this`);
}

// --- Running --------------------------------------------------------------

export function featureFiles(paths) {
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
  const failures = stepsDrift(nativeSteps(readFileSync(join(root, NATIVE), "utf8")));
  for (const drift of failures) console.log(`✘ steps: ${drift}`);
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
      // A refused write left over is that scenario's own business.
      await world.studio?.close().catch(() => {});
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
