// Studio in a dedicated worker, so it may wait for a preview page's answer
// (`preview.js`) without holding up the page. The page talks to it by
// message: `{id, open: {backend, files?, caps, buffer?, timeoutMs?}}` once,
// then `{id, call, args, client?}`, `{id, stored: {path, text}}` (a file another
// writer changed in storage), `{id, files: true}` (what storage holds),
// `{id, text: doc}` and `{id, replaceText: [doc, generation, start, end,
// text]}` (a person's typing in the code view);
// each is answered `{id, result}` or
// `{id, failure, refusal?}`.

import { openStudio } from "./studio.js";
import { memoryBackend, openBackend } from "./store.js";
import { TAG, workerAsk } from "./preview.js";

let studio;
let backend;

self.addEventListener("message", async ({ data }) => {
  if (data?.[TAG] !== undefined) return;
  const { id, open, call, args, client, stored, files, text, replaceText } = data;
  try {
    if (open) {
      backend = open.backend === "memory" ? memoryBackend(open.files) : await openBackend(open.backend);
      const ask = open.buffer && workerAsk(open.buffer, (m) => self.postMessage(m), { timeoutMs: open.timeoutMs });
      const replaced = studio;
      studio = await openStudio({ backend, caps: open.caps, ask });
      // The open worked, and every later call goes to the new Studio: a
      // refusal of the old one's leftover writes is the old project's, told
      // on its own, not the open's answer (as `Workspace.js` does).
      await replaced?.close().catch((e) => console.error(e));
      self.postMessage({ id, result: { place: backend.place } });
    } else if (stored) {
      await (stored.text === null ? backend.remove(stored.path) : backend.write(stored.path, stored.text));
      studio.stored(stored.path, stored.text);
      self.postMessage({ id, result: true });
    } else if (text !== undefined) {
      self.postMessage({ id, result: studio.text(text) });
    } else if (replaceText) {
      self.postMessage({ id, result: (await studio.replaceText(...replaceText)) ?? null });
    } else if (files) {
      self.postMessage({ id, result: await backend.load() });
    } else {
      self.postMessage({ id, result: await studio.call(call, args, client) });
    }
  } catch (e) {
    self.postMessage({ id, failure: String(e?.message ?? e), refusal: e?.refusal });
  }
});
