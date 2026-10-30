// The OPFS backend over a stand-in for the origin's private file system,
// which refuses what the File System API says it refuses: `features:web`
// runs in node, which has no OPFS, and `memoryBackend` cannot show what
// removing a file storage never had does. `web/browser.mjs` checks the
// same against Chromium's.
//
//   node web/opfs.mjs   (after `mise run web:build`)

import { readFileSync } from "node:fs";
import { openStudio } from "./studio.js";
import { opfsBackend } from "./store.js";

const wasm = readFileSync(new URL("./pkg/studio_web_bg.wasm", import.meta.url));

const notFound = (what) => new DOMException(`${what} was not found`, "NotFoundError");

class StandInDirectory {
  kind = "directory";
  entries_ = new Map();
  refuse = () => null;

  async getDirectoryHandle(name, { create = false } = {}) {
    if (!this.entries_.has(name)) {
      if (!create) throw notFound(name);
      const dir = new StandInDirectory();
      dir.refuse = this.refuse;
      this.entries_.set(name, dir);
    }
    return this.entries_.get(name);
  }

  async getFileHandle(name, { create = false } = {}) {
    if (!this.entries_.has(name)) {
      if (!create) throw notFound(name);
      // Over quota, a file cannot be made either.
      const refused = this.refuse(name);
      if (refused) throw refused;
      let text = "";
      this.entries_.set(name, {
        kind: "file",
        getFile: async () => ({ text: async () => text }),
        createWritable: async () => {
          let next = "";
          return {
            write: async (written) => {
              next = written;
            },
            close: async () => {
              text = next;
            },
            abort: async () => {},
          };
        },
      });
    }
    return this.entries_.get(name);
  }

  async removeEntry(name) {
    if (!this.entries_.delete(name)) throw notFound(name);
  }

  async *entries() {
    yield* this.entries_;
  }
}

const checks = [];
const check = (what, ok, detail) => checks.push([what, ok, detail]);
const refusedBy = async (call) => {
  try {
    await call();
    return null;
  } catch (e) {
    return e;
  }
};

// Removing what is not there: the file, and the directory it would be in.
{
  const backend = await opfsBackend("project", new StandInDirectory());
  const missing = await refusedBy(() => backend.remove("Never.qml"));
  check("removing a file storage never had is done", missing === null, String(missing));
  const noDir = await refusedBy(() => backend.remove("gone/too/Never.qml"));
  check("so is removing one from a directory it never had", noDir === null, String(noDir));
}

// Any other refusal is still one.
{
  const root = new StandInDirectory();
  const backend = await opfsBackend("project", root);
  const top = await root.getDirectoryHandle("project");
  top.removeEntry = async () => {
    throw new DOMException("the file is in use", "NoModificationAllowedError");
  };
  const refused = await refusedBy(() => backend.remove("Busy.qml"));
  check("a removal storage refuses for another reason still refuses", refused?.name === "NoModificationAllowedError", String(refused));
}

// The review's sequence, through a Studio: a refused create, its undo, and
// every call after.
{
  const root = new StandInDirectory();
  const backend = await opfsBackend("project", root);
  await backend.write("Main.qml", "import QtQuick\n\nItem {\n    id: root\n    width: 10\n}\n");
  const studio = await openStudio({ backend, wasm });
  const top = await root.getDirectoryHandle("project");
  top.refuse = (name) => (name === "New.qml" ? new DOMException("over quota", "QuotaExceededError") : null);
  const create = await refusedBy(() => studio.call("document_new", { doc: "New.qml" }));
  check("the refused create refuses its call", create?.refusal?.details?.kind === "not_stored", String(create));
  top.refuse = () => null;
  const undo = await refusedBy(async () => {
    const answer = await studio.call("history", { action: "undo" });
    if (answer.error) throw new Error(JSON.stringify(answer.error));
  });
  check("the undo that takes the file back away is answered", undo === null, String(undo));
  const next = await refusedBy(() => studio.call("property_set", { doc: "Main.qml", node: { id: "root" }, name: "width", value: 20 }));
  check("and so is the call after it", next === null, String(next));
  const kept = (await backend.load())["Main.qml"];
  check("whose edit is in storage", kept.includes("width: 20"), kept);
}

for (const [what, ok, detail] of checks) console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
if (checks.some(([, ok]) => !ok)) process.exitCode = 1;
