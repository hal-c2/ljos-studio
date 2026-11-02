// `Studio.Workspace` as a type of ljos-web's runtime: the face the native
// block (`crates/studio_blocks`) has on the desktop, over the session in
// wasm (`studio.js`). ljos-web will generate faces from the Rust
// (docs/LJOS_WEB.md in ljos-web, sections 1–3); until it does, this one is
// written by hand, as its `wasm-counter` spike's is, and stands where the
// compiler's stand-in (`standins/Studio/Workspace.qml`) is asked for.
//
// It does what the native block does, in the same order: a gesture is a
// tool call, one at a time per workspace; after each, and after any write
// the session tells of (an agent's, a preview's), the workspace reads the
// session's view (`studio_mcp::view`, the same projection) and publishes
// it. Nothing polls.
import { onCleanup, untrack } from "solid-js";
import { defineType, kinds, onChange, QtObject, settle, slot, typed } from "ljos-web/object";

// The session the workspaces of this page show, once `serve` is called.
let served = null;
const live = new Set();

// Gives this page's workspaces `studio` (an `openStudio` one), as the
// native `studio_blocks::serve` does. A workspace made before has no
// session to read and refuses every gesture until then. The Studio this
// replaces is closed: nothing else here reaches it.
export function serve(studio) {
  // Storage's refusal of what it had left has no call left to tell.
  if (served !== studio) served?.close().catch((e) => console.error(e));
  served = studio;
  studio.watch(() => {
    for (const ws of live) if (ws.published < studio.writes) ws.enqueue({ refresh: true });
  });
  for (const ws of live) ws.enqueue({ refresh: true });
}

// The workspaces of this page, for a check that calls what no gesture reaches
// on a page with no preview.
export const workspaces = () => [...live].map((queue) => queue.self);

// The view's parts, as the workspace's properties.
const PARTS = ["documents", "generation", "text", "layers", "selected", "selection", "inspector", "states", "roles", "flow", "palette", "done", "undone", "problem", "previews", "theme", "tokens", "comments", "leases", "blank", "chosen", "toolkit"];

export const Workspace = defineType("Workspace", QtObject, {
  properties: {
    documents: null,
    doc: typed(kinds.string, ""),
    generation: typed(kinds.real, 0),
    text: typed(kinds.string, ""),
    layers: null,
    selected: typed(kinds.string, ""),
    selection: null,
    inspector: null,
    states: null,
    // What a panel reads before the first view is in starts empty, as the
    // native block's does (`studio_blocks`, `Default for Workspace`).
    roles: [],
    flow: { states: [], edges: [] },
    palette: null,
    done: null,
    undone: null,
    preview: null,
    error: typed(kinds.string, ""),
    problem: typed(kinds.string, ""),
    previews: typed(kinds.bool, false),
    theme: typed(kinds.string, ""),
    tokens: [],
    comments: [],
    // What other writers hold: `doc`, `node`, `holder`, `ends_in_ms`.
    leases: [],
    blank: typed(kinds.bool, false),
    chosen: [],
    toolkit: [],
    rootStates: [],
    // The page's preview is the page's own, so nothing here is ever waiting
    // on a picture to settle.
    settling: typed(kinds.bool, false),
  },
  signals: ["answered", "refused"],
  methods: {
    // Calls the Studio tool `tool` with `args`, after the calls before it;
    // `answered` or `refused` tells how it went.
    dispatch(tool, args) {
      this.$queue.enqueue({ tool, args: args ?? {} });
    },
    // Whether `text` is a literal as QML reads one, as the native block says.
    literal(text) {
      return served?.isLiteral(text) ?? false;
    },
    // A call that follows from what the person just did: it leaves the refusal
    // on the line, and says its own only when the line is empty.
    follow(tool, args) {
      this.$queue.enqueue({ tool, args: args ?? {}, follows: true });
    },
    lookAgain() {},
  },
  setup(self) {
    const queue = new Queue(self);
    self.$queue = queue;
    live.add(queue);
    onCleanup(() => live.delete(queue));
    // The first value is there once the objects being made all are: the
    // native block's `on_complete`. Every later one is the shell moving to
    // another document.
    onChange(
      self,
      "doc",
      () => queue.enqueue({ refresh: true }),
      () => served && queue.enqueue({ refresh: true }),
    );
  },
});

export default Workspace;

class Queue {
  jobs = [];
  busy = false;
  // The session's write count the published view was read at.
  published = 0;
  #self;

  constructor(self) {
    this.#self = self;
  }

  get self() {
    return this.#self;
  }

  enqueue(job) {
    // A refresh reads the whole session, so one waiting is enough.
    if (job.refresh && this.jobs.some((j) => j.refresh)) return;
    this.jobs.push(job);
    this.#pump();
  }

  async #pump() {
    if (this.busy || this.jobs.length === 0) return;
    const job = this.jobs.shift();
    const self = this.#self;
    if (!served) {
      if (job.tool) self.refused(job.tool, { code: "unavailable", message: "no Studio session is served" });
      return this.#pump();
    }
    this.busy = true;
    try {
      let answer;
      if (job.tool) {
        try {
          // The window is the person: its Undo takes the person's edits.
          answer = await served.call(job.tool, job.args, "person");
        } catch (e) {
          // Storage refused what the call wrote (`studio.js`).
          answer = { error: e.refusal ?? { code: "internal", message: String(e?.message ?? e) } };
        }
        if (answer.ok !== undefined) this.#rebase(job, answer.ok);
      }
      const doc = untrack(() => self.doc);
      const seen = served.writes;
      this.#publish(served.view(doc), seen);
      if (job.tool) {
        if (answer.error) {
          if (!job.follows || !untrack(() => self.error)) this.#give("error", answer.error.message ?? "");
          self.refused(job.tool, answer.error);
        } else {
          if (!job.follows) this.#give("error", "");
          self.answered(job.tool, answer.ok);
        }
      }
    } finally {
      this.busy = false;
    }
    this.#pump();
  }

  // A person types on in the code view while their last edit is on its
  // way: the edits waiting on it are based on its answer's generation, as
  // the native block's `rebase` has it. One based on anything else (an
  // agent wrote in between) is left to be refused.
  #rebase(job, answer) {
    if (job.tool !== "document_edit") return;
    const from = job.args.generation;
    const to = answer?.generation;
    if (typeof from !== "number" || typeof to !== "number") return;
    for (const next of this.jobs) {
      if (next.tool === "document_edit" && next.args.doc === job.args.doc && next.args.generation === from) {
        next.args = { ...next.args, generation: to };
      }
    }
  }

  #wake = null;
  #wakeAt = null;

  #publish(view, seen) {
    const self = this.#self;
    // The shell may have moved to another document while this was read.
    if (untrack(() => self.doc) !== view.doc) return;
    this.published = Math.max(this.published, seen);
    for (const part of PARTS) this.#give(part, view[part]);
    this.#give("rootStates", view.root_states);
    // A lease ends with no write to tell of it: read the session again
    // then, as the native block's `wake_after` does. One wake at a time,
    // the earliest.
    const ends = Math.min(...(view.leases ?? []).map((l) => l.ends_in_ms));
    if (Number.isFinite(ends)) {
      const at = Date.now() + ends;
      if (this.#wakeAt === null || at < this.#wakeAt) {
        clearTimeout(this.#wake);
        this.#wakeAt = at;
        this.#wake = setTimeout(() => {
          this.#wakeAt = null;
          this.enqueue({ refresh: true });
        }, ends);
      }
    }
    const p = view.preview;
    // The canvas shows the frame itself, laid over it, so `picture` stays 0.
    this.#give(
      "preview",
      p && p.doc === view.doc
        ? { target: p.target, width: p.width, height: p.height, generation: p.generation, state: p.state, picture: 0 }
        : null,
    );
  }

  // A value the type gives, not an assignment: a binding on it stands.
  #give(name, value) {
    slot(this.#self, name).provide(value);
    settle();
  }
}
