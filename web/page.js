// A preview page: the frame Studio's previews run in, answering
// `crates/studio_web/src/protocol.rs` over `postMessage`.
//
// It does what ljos-web lets a page do today. ljos-web compiles QML ahead of
// time, natively (`qmlc`), so a page given QML text can run it only if it
// is given a compiler (`compile`); without one `open` answers that it could
// not start. The calls ljos-web has no way to answer yet are refused with
// `unsupported` and the reason (DESIGN.md section 7, S5, requests).

import { TAG, VERSION } from "./preview.js";

// `element` is where the preview is mounted; `mount(Component, element)`
// is ljos-web's (`ljos-web/object`); `compile(files, entry, aliases)`
// gives a component of the entry, when this page has a QML compiler.
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

  const unsupported = (call, why) => ({ answer: "refused", code: "unsupported", message: `${call}: ${why}` });

  const start = async () => {
    const Component = await compile(files, entry, aliases);
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
    screenshot: () => unsupported("screenshot", "a page cannot draw its DOM into a picture; ljos-web has no capture of a mounted root"),
    outline: () => unsupported("outline", "ljos-web exposes no tree of a mounted root's objects"),
    read: () => unsupported("read", "ljos-web keeps a component's ids private to its context"),
    write: () => unsupported("write", "ljos-web keeps a component's ids private to its context"),
    logs(r) {
      const first = Math.max(r.since - dropped, 0);
      return { answer: "logs", lines: logs.slice(first), next: dropped + logs.length };
    },
    pick: () => unsupported("pick", "ljos-web's objects and elements carry no place in the QML they were declared in"),
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
      answer = answers[call] ? await answers[call](message.request) : unsupported(call, "not a call of this protocol");
    } catch (e) {
      answer = { answer: "refused", code: "app_faulted", message: String(e?.message ?? e) };
    }
    parent.postMessage({ [TAG]: VERSION, id: message.id, answer }, "*");
  });
}
