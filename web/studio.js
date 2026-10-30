// Studio's MCP tools for a page: `call(tool, args)` answers as an MCP
// client's call does, once storage has what the call wrote.

import init, { StudioCore } from "./pkg/studio_web.js";

let started;

// `backend` from `store.js`; `caps` as `--caps` takes them; `ask`, from
// `preview.js`, when this page shows previews; `wasm` the module's bytes
// or URL (node passes bytes, a page may leave it to the default URL).
export async function openStudio({ backend, caps = "inspect,mutate", ask, wasm } = {}) {
  started ??= init(wasm === undefined ? undefined : { module_or_path: wasm });
  await started;
  const files = await backend.load();
  return new Studio(new StudioCore(backend.place, JSON.stringify(files), caps, ask), backend);
}

export class Studio {
  #core;
  #backend;
  // Storage is told in the order the calls wrote, one call's files after
  // the last's.
  #stored = Promise.resolve();

  constructor(core, backend) {
    this.#core = core;
    this.#backend = backend;
    core.watch(() => this.#heard());
  }

  // `{ok, text, images}` or `{error: {code, message, details}}`, the
  // tool's own refusal. Rejects only when storage refused what the call
  // wrote; the error's `refusal` says which files.
  async call(tool, args = {}) {
    const answer = JSON.parse(this.#core.call(tool, JSON.stringify(args)));
    await this.#store();
    return answer;
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
