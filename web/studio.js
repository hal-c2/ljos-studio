// Studio's MCP tools for a page: `call(tool, args)` answers as an MCP
// client's call does, once storage has what the call wrote.

import init, { isLiteral, StudioCore } from "./pkg/studio_web.js";

let started;

// `backend` from `store.js`; `caps` as `--caps` takes them; `ask`, from
// `preview.js`, when this page shows previews; `wasm` the module's bytes
// or URL (node passes bytes, a page may leave it to the default URL).
export async function openStudio({ backend, caps = "inspect,mutate", ask, wasm } = {}) {
  // A module that did not load is loaded again by the next open, not
  // refused until the page reloads.
  started ??= init(wasm === undefined ? undefined : { module_or_path: wasm }).catch((e) => {
    started = undefined;
    throw e;
  });
  await started;
  const files = await backend.load();
  return new Studio(new StudioCore(backend.place, JSON.stringify(files), caps, ask), backend);
}

// How many functions this page's Studios still hold (`Studio.close`).
export { heldFunctions } from "./pkg/studio_web.js";

export class Studio {
  #core;
  #backend;
  // Storage is told in the order the calls wrote, one call's files after
  // the last's.
  #stored = Promise.resolve();
  #leaving = (e) => {
    // A page kept to come back to (`persisted`) still needs its Studio.
    if (!e.persisted) this.close().catch(() => {});
  };

  constructor(core, backend) {
    this.#core = core;
    this.#backend = backend;
    core.watch(() => this.#heard());
    globalThis.addEventListener?.("pagehide", this.#leaving);
  }

  // Stores what is still to be stored, then lets go of the session and of
  // the functions the page gave it, `ask` and `watch`'s. Without this a
  // Studio lives as long as the wasm instance: the instance holds the
  // watcher, the watcher holds this. A page replacing a Studio closes the
  // old one. Rejects as a call does if storage refused what was left, and
  // is closed all the same. After it `call` answers a refusal with
  // `details.kind = "closed"`, and the other methods throw.
  async close() {
    globalThis.removeEventListener?.("pagehide", this.#leaving);
    this.#watchers = [];
    try {
      await this.#store();
    } finally {
      this.#core.close();
    }
  }

  // `{ok, text, images}` or `{error: {code, message, details}}`, the
  // tool's own refusal. Rejects only when storage refused what the call
  // wrote; the error's `refusal` says which files. `client` is the name the
  // caller gave itself, one writer to the session (a window's is
  // "person"); left out, the call is an unnamed agent's.
  async call(tool, args = {}, client = undefined) {
    const answer = JSON.parse(this.#core.call(tool, JSON.stringify(args), client));
    await this.#store();
    return answer;
  }

  // Whether `text` is a literal as QML reads one.
  isLiteral(text) {
    return isLiteral(text);
  }

  // The tools offered, as `tools/list` gives them.
  tools() {
    return JSON.parse(this.#core.tools());
  }

  // What a window showing `doc` sees of the session: the state
  // `Studio.Workspace` publishes (`Workspace.js`).
  view(doc) {
    return JSON.parse(this.#core.view(doc));
  }

  // How many writes the session has told of, as they are made: a view
  // read when it was `n` shows every write up to `n`.
  writes = 0;
  #watchers = [];
  #told = false;

  // `stale()` after every write to the session, a call's or a preview's,
  // in a microtask: the write is still being made when the session tells.
  // Several writes in one task are one `stale()`.
  watch(stale) {
    this.#watchers.push(stale);
  }

  #heard() {
    this.writes++;
    if (this.#told) return;
    this.#told = true;
    queueMicrotask(() => {
      this.#told = false;
      for (const stale of this.#watchers) stale();
    });
  }

  // Storage has `text` for `path` (`null`: no such file) from somewhere
  // other than this session, such as another tab: what the tools read from
  // now on, as a native project sees a file written into its directory.
  stored(path, text) {
    this.#core.stored(JSON.stringify({ path, text }));
  }

  // A document as a code view shows it: `{text, generation}`.
  text(doc) {
    return JSON.parse(this.#core.text(doc));
  }

  // A person's typing in a code view; offsets as JavaScript counts them.
  async replaceText(doc, generation, start, end, text) {
    this.#core.replace_text(doc, generation, start, end, text);
    await this.#store();
  }

  #store() {
    const pending = JSON.parse(this.#core.take());
    if (pending.length === 0) return this.#stored;
    this.#stored = this.#stored.then(async () => {
      const failed = [];
      for (const p of pending) {
        try {
          if (p.text === null) await this.#backend.remove(p.path);
          else await this.#backend.write(p.path, p.text);
        } catch (e) {
          failed.push({ ...p, reason: String(e?.message ?? e) });
        }
      }
      if (failed.length === 0) return;
      // Done again with the next call's, as the session holds the file then.
      this.#core.unstored(JSON.stringify(failed.map(({ path }) => ({ path, text: null }))));
      const error = new Error(`storage did not keep ${failed.map((f) => f.path).join(", ")}`);
      error.refusal = {
        code: "internal",
        message: error.message,
        details: { kind: "not_stored", files: failed.map(({ path, reason }) => ({ path, reason })) },
      };
      throw error;
    });
    // A refusal belongs to the call that made it, not to the next.
    const mine = this.#stored;
    this.#stored = mine.catch(() => {});
    return mine;
  }
}
