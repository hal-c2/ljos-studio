// A preview page: the frame Studio's previews run in, answering
// `crates/studio_web/src/protocol.rs` over `postMessage`.
//
// It does what ljos-web lets a page do today. A page given QML text runs it
// through a Compiler (`compiler.js`); without one `open` answers that it
// could not start. A call ljos-web has no way to answer yet is answered
// `not_supported` with the numbered ljos-web request it waits on (DESIGN.md,
// S5), never with a made-up answer.

import { TAG, VERSION } from "./preview.js";

// `element` is where the preview is mounted; `mount(Component, element)`
// is ljos-web's (`ljos-web/object`); `compile` is a Compiler
// (`compiler.js`), when this page has one.
export function servePreview({ element, mount, compile, window: w = globalThis, parent = w.parent }) {
  let files = null;
  let entry = null;
  let aliases = [];
  let running = null;
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

  // What the compiler said, as one message a person reads.
  const said = (diagnostics) =>
    diagnostics.map((d) => (d.file ? `${d.file}${d.line ? `:${d.line}:${d.column ?? 1}` : ""}: ${d.message}` : d.message)).join("\n");

  // A remount: the app's state starts over (ljos-web has no reload that
  // keeps a mounted root's state, request 18).
  const start = async () => {
    const compiled = await compile({ files, entry, aliases });
    if (compiled.diagnostics) throw new Error(said(compiled.diagnostics));
    const Component = compiled.component;
    running?.dispose?.();
    element.replaceChildren();
    running = mount(Component, element);
  };

  const apply = (changed) => {
    for (const [path, text] of Object.entries(changed)) {
      if (text === null) delete files[path];
      else files[path] = text;
    }
  };

  const reload = async (changed) => {
    apply(changed);
    if (!compile) return { answer: "reload", outcome: "failed", message: "this page has no QML compiler" };
    try {
      await start();
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
      return reload(r.changed);
    },
    resize(r) {
      element.style.width = `${r.size.width}px`;
      element.style.height = `${r.size.height}px`;
      return { answer: "done" };
    },
    screenshot: () => waiting("screenshot", "a page cannot draw its DOM into a picture; ljos-web has no capture of a mounted root", 2),
    // The DOM's tree is not the QML object tree, so it is not given as one.
    outline: () => waiting("outline", "ljos-web exposes no tree of a mounted root's objects", 3),
    read: () => waiting("read", "ljos-web keeps a component's ids private to its context", 4),
    write: () => waiting("write", "ljos-web keeps a component's ids private to its context", 4),
    logs(r) {
      const first = Math.max(r.since - dropped, 0);
      return { answer: "logs", lines: logs.slice(first), next: dropped + logs.length };
    },
    pick: () => waiting("pick", "ljos-web's objects and elements carry no place in the QML they were declared in", 5),
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
