// A preview page: the frame Studio's previews run in, answering
// `crates/studio_web/src/protocol.rs` over `postMessage`.
//
// A page given QML text runs it through a Compiler (`compiler.js`); without
// one `open` answers that it could not start. What is asked of the running
// app is `ljos-web/inspect`'s to answer. A page built without it refuses as
// `not_supported`, never with a made-up answer. One whose app failed to start
// has nothing to answer for, and says that instead: a save can mend it.

import { TAG, VERSION } from "./preview.js";

// What a program reads by address, as ljos-web's site builder keeps as a file.
const DATA = /\.(json|txt|csv|xml)$/;
const COMPILED_FROM = /(^|\/)(theme|ljostypes)\.json$/;

// `element` is where the preview is mounted; `mount(Component, element)`
// is ljos-web's (`ljos-web/object`); `compile` is a Compiler
// (`compiler.js`), when this page has one.
export function servePreview({ element, mount, reload: remade, inspect, tell, compile, window: w = globalThis, parent = w.parent }) {
  let files = null;
  let entry = null;
  let aliases = [];
  let running = null;
  let inspector = null;
  // The address the running compile's files are at, and the data files
  // already told there.
  let base = null;
  // The aliases of the compile that is running: a reload that throws leaves it.
  let compiledWith = [];
  const told = new Set();
  let size = null;
  // An item root is laid out at the preview's size, as `ljos run --size`
  // lays it out; a Window fills the element itself.
  const layOut = () => {
    const root = running?.object;
    if (!size || !root?.$node || root.$contentItem) return;
    root.width = size.width;
    root.height = size.height;
  };
  // A file is named by its path in the project, not the compile's: an aliased
  // module's files are compiled at the module's path, so `served` read backwards.
  const inProject = (file) => {
    const path = file?.replace(/^projects\/\d+\//, "") ?? null;
    if (path === null) return null;
    // Only a module's own files move: one nested in it is compiled where it is.
    const dir = path.slice(0, path.lastIndexOf("/"));
    const alias = compiledWith.find((a) => a.module.replaceAll(".", "/") === dir);
    return alias ? `${alias.dir}/${path.slice(alias.module.length + 1)}` : path;
  };
  let generation = 0;
  const logs = [];
  let dropped = 0;

  // The previewed app's console lines, without file or line: ljos-web's
  // runtime does not say where a line was logged from.
  for (const level of ["debug", "log", "info", "warn", "error"]) {
    const original = w.console[level].bind(w.console);
    w.console[level] = (...args) => {
      logs.push({ level: level === "log" ? "info" : level, message: args.map(String).join(" ") });
      if (logs.length > 1000) {
        logs.shift();
        dropped += 1;
      }
      original(...args);
    };
  }

  const waiting = (call, why, request) => ({ answer: "not_supported", message: `${call}: ${why}`, ljos_web_request: request });

  // No inspector to ask. A page that compiles and has no root has an app that
  // failed to start, which no ljos-web request will change; the save that mends
  // it starts it again.
  const unanswered = (call, why, request) =>
    compile && !running
      ? { answer: "refused", code: "not_running", message: `${call}: the app failed to start; saving a fix starts it again` }
      : waiting(call, why, request);

  // What the compiler said, as one message a person reads.
  const said = (diagnostics) =>
    diagnostics.map((d) => (d.file ? `${d.file}${d.line ? `:${d.line}:${d.column ?? 1}` : ""}: ${d.message}` : d.message)).join("\n");

  const sameAliases = (one, other) =>
    one.length === other.length && one.every((a) => other.some((b) => b.module === a.module && b.dir === a.dir));

  // A reload keeps the app's state where it can, as `ljos run --watch` does;
  // only a page with no `reload` (a test's) starts over.
  const start = async ({ again = false } = {}) => {
    const compiled = await compile({ files, entry, aliases });
    if (compiled.diagnostics) throw new Error(said(compiled.diagnostics));
    const Component = compiled.component;
    // Compared with what is running, not with this call: a switch that did not
    // compile has changed `aliases`, and the save that mends it is still the
    // first of the new program.
    if (again && remade && running && sameAliases(aliases, compiledWith)) {
      remade(running, Component);
    } else {
      running?.dispose?.();
      element.replaceChildren();
      // Before the mount, which can throw: a disposed root is no root to
      // reload or to answer for, and the next save has to mount.
      running = inspector = base = null;
      running = mount(Component, element, {}, { reloadable: Boolean(remade) });
    }
    // Only now: a reload that throws leaves the old root running, and its
    // readers still watch the old compile's address.
    base = compiled.base ?? null;
    compiledWith = aliases;
    told.clear();
    layOut();
    inspector = inspect && running?.object ? inspect(running.object) : null;
  };

  const apply = (changed) => {
    for (const [path, text] of Object.entries(changed)) {
      if (text === null) delete files[path];
      else files[path] = text;
    }
  };

  // A file only read as data is read again by what reads it, with nothing
  // compiled or remade, as the native preview's own watch does. What is
  // compiled from (a theme, a type dump) is not data, and a removed file is
  // not read again from nowhere: those reload.
  const onlyData = (changed) =>
    Boolean(tell && base && running) &&
    Object.entries(changed).every(([path, text]) => text !== null && DATA.test(path) && !COMPILED_FROM.test(path));

  // Where a data file is served: an aliased module's files are at the
  // module's path, and what they replaced is not served at all.
  const served = (path) => {
    const parts = path.split("/");
    const file = parts.pop();
    const dir = parts.join("/");
    const alias = compiledWith.find((a) => a.dir === dir);
    if (alias) return `${alias.module.replaceAll(".", "/")}/${file}`;
    return compiledWith.some((a) => a.module.replaceAll(".", "/") === dir) ? null : path;
  };

  const tellData = (changed) => {
    for (const [path, text] of Object.entries(changed)) {
      const at = served(path);
      if (at === null) continue;
      const url = new URL(at, new URL(base, w.location?.href)).href;
      tell(url, text, told.has(url));
      told.add(url);
    }
  };

  const reload = async (changed, { again = true } = {}) => {
    apply(changed);
    if (!compile) return { answer: "reload", outcome: "failed", message: "this page has no QML compiler" };
    if (again && onlyData(changed)) {
      tellData(changed);
      return { answer: "reload", outcome: "unchanged" };
    }
    try {
      await start({ again });
    } catch (e) {
      return { answer: "reload", outcome: "failed", message: String(e?.message ?? e) };
    }
    generation += 1;
    return { answer: "reload", outcome: "applied", generation };
  };

  const answers = {
    async open(r) {
      files = { ...r.files };
      entry = r.entry;
      aliases = r.aliases;
      generation = 0;
      answers.resize(r);
      if (!compile) {
        return {
          answer: "not_started",
          message: "this page has no QML compiler: ljos-web compiles QML ahead of time, natively (qmlc)",
        };
      }
      try {
        await start();
      } catch (e) {
        return { answer: "not_started", message: String(e?.message ?? e) };
      }
      return { answer: "done" };
    },
    reload: (r) => reload(r.changed),
    alias(r) {
      aliases = aliases.filter((a) => a.module !== r.module);
      if (r.dir !== null) aliases.push({ module: r.module, dir: r.dir });
      // Another module's code is another program: what the old one's objects
      // were given says nothing of the new one's, so it starts as a program does.
      return reload(r.changed, { again: false });
    },
    resize(r) {
      element.style.width = `${r.size.width}px`;
      element.style.height = `${r.size.height}px`;
      size = r.size;
      layOut();
      return { answer: "done" };
    },
    async screenshot(r) {
      if (!inspector) return unanswered("screenshot", "a page cannot draw its DOM into a picture; ljos-web has no capture of a mounted root", 2);
      const taken = await inspector.screenshot({ max_size: r.max_side });
      if (taken.error) return { answer: "refused", code: taken.error, message: taken.message };
      let text = "";
      for (let at = 0; at < taken.png.length; at += 0x8000) text += String.fromCharCode(...taken.png.subarray(at, at + 0x8000));
      return { answer: "picture", png: btoa(text), width: taken.width, height: taken.height };
    },
    outline: () => (inspector ? { answer: "outline", text: inspector.outline().text } : unanswered("outline", "ljos-web exposes no tree of a mounted root's objects", 3)),
    read(r) {
      if (!inspector) return unanswered("read", "ljos-web keeps a component's ids private to its context", 4);
      const values = inspector.read(r.id, r.names);
      return values ? { answer: "values", values } : { answer: "no_object", id: r.id };
    },
    write(r) {
      if (!inspector) return unanswered("write", "ljos-web keeps a component's ids private to its context", 4);
      if (inspector.write(r.id, r.name, r.value)) return { answer: "done" };
      // Both come back false, and only one of them means the object is not there.
      if (!inspector.find(r.id)) return { answer: "no_object", id: r.id };
      return { answer: "refused", code: "no_property", message: `\`${r.id}\` has no property \`${r.name}\`` };
    },
    click(r) {
      if (!inspector) return unanswered("click", "ljos-web keeps a component's ids private to its context", 4);
      const done = inspector.click(r.at ? { at: r.at } : r.id);
      if (done.error === "not_found") return { answer: "no_object", id: r.id };
      if (done.error) return { answer: "refused", code: done.error, message: done.message };
      return { answer: "done" };
    },
    logs(r) {
      const first = Math.max(r.since - dropped, 0);
      return { answer: "logs", lines: logs.slice(first), next: dropped + logs.length };
    },
    pick(r) {
      if (!inspector) return unanswered("pick", "ljos-web's objects and elements carry no place in the QML they were declared in", 5);
      const source = inspector.pick(r.x, r.y)?.source;
      return { answer: "picked", declared: source ? { ...source, file: inProject(source.file) } : null };
    },
    close() {
      running?.dispose?.();
      element.replaceChildren();
      running = files = null;
      return { answer: "done" };
    },
  };

  w.addEventListener("message", async (event) => {
    const message = event.data;
    if (event.source !== parent || message?.[TAG] !== VERSION) return;
    const { call } = message.request;
    let answer;
    try {
      answer = answers[call] ? await answers[call](message.request) : { answer: "refused", code: "unsupported", message: `${call}: not a call of this protocol` };
    } catch (e) {
      answer = { answer: "refused", code: "app_faulted", message: String(e?.message ?? e) };
    }
    parent.postMessage({ [TAG]: VERSION, id: message.id, answer }, "*");
  });
}
