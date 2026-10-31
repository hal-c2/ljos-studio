// Studio in a dedicated worker, so it may wait for a preview page's answer
// (`preview.js`) without holding up the page. The page talks to it by
// message: `{id, open: {backend, files?, caps, buffer?, timeoutMs?}}` once,
// then `{id, call, args}`; each is answered `{id, result}` or
// `{id, failure, refusal?}`.

import { openStudio } from "./studio.js";
import { memoryBackend, openBackend } from "./store.js";
import { TAG, workerAsk } from "./preview.js";

let studio;

self.addEventListener("message", async ({ data }) => {
  if (data?.[TAG] !== undefined) return;
  const { id, open, call, args } = data;
  try {
    if (open) {
      const backend = open.backend === "memory" ? memoryBackend(open.files) : await openBackend(open.backend);
      const ask = open.buffer && workerAsk(open.buffer, (m) => self.postMessage(m), { timeoutMs: open.timeoutMs });
      const replaced = studio;
      studio = await openStudio({ backend, caps: open.caps, ask });
      // The open worked, and every later call goes to the new Studio: a
      // refusal of the old one's leftover writes is the old project's, told
      // on its own, not the open's answer (as `Workspace.js` does).
      await replaced?.close().catch((e) => console.error(e));
      self.postMessage({ id, result: { place: backend.place } });
    } else {
      self.postMessage({ id, result: await studio.call(call, args) });
    }
  } catch (e) {
    self.postMessage({ id, failure: String(e?.message ?? e), refusal: e?.refusal });
  }
});
