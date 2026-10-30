// `worker.js` as a page drives it, by message: an open that replaces a
// Studio is answered for the new one even when the old one's close is
// refused, as the open worked and every later call goes to the new one.
//
//   node web/worker.mjs   (after `mise run web:build`)

import { readFileSync } from "node:fs";
import { Studio, openStudio } from "./studio.js";
import { memoryBackend } from "./store.js";

const wasm = readFileSync(new URL("./pkg/studio_web_bg.wasm", import.meta.url));
// The worker loads the module from its default URL, which node does not
// fetch: loaded here from the bytes, once for the whole page, as a page's
// first open would.
await (await openStudio({ backend: memoryBackend({}), wasm })).close();

const answers = new Map();
globalThis.self = Object.assign(new EventTarget(), {
  postMessage: (message) => answers.get(message.id)?.(message),
});
await import("./worker.js");
let next = 0;
const send = (message) =>
  new Promise((resolve) => {
    const id = ++next;
    answers.set(id, resolve);
    self.dispatchEvent(new MessageEvent("message", { data: { id, ...message } }));
  });

const main = (width) => `import QtQuick\n\nItem {\n    id: root\n    width: ${width}\n}\n`;
const checks = [];
const first = await send({ open: { backend: "memory", files: { "Main.qml": main(10) }, caps: "inspect,mutate" } });
checks.push(["the first open is answered", first.result?.place === "memory", first]);

// The old Studio's close rejects as it does when storage refuses what it
// had left to store (`cores.mjs`), after closing it all the same.
const close = Studio.prototype.close;
Studio.prototype.close = async function () {
  await close.call(this);
  const error = new Error("storage did not keep Main.qml");
  error.refusal = { code: "internal", message: error.message, details: { kind: "not_stored", files: [{ path: "Main.qml" }] } };
  throw error;
};
const logged = [];
const consoleError = console.error;
console.error = (...args) => logged.push(args.map(String).join(" "));
let second;
try {
  second = await send({ open: { backend: "memory", files: { "Other.qml": main(20) }, caps: "inspect,mutate" } });
} finally {
  Studio.prototype.close = close;
  console.error = consoleError;
}
checks.push(["an open whose old Studio's close is refused is answered as the open it is", second.result?.place === "memory" && second.failure === undefined, second]);
checks.push(["the old Studio's refusal is told on its own", logged.some((l) => l.includes("storage did not keep Main.qml")), logged]);
const tree = await send({ call: "document_tree", args: { doc: "Other.qml" } });
checks.push(["calls go to the new Studio", tree.result?.ok?.nodes !== undefined, tree]);

for (const [what, ok, detail] of checks) console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
if (checks.some(([, ok]) => !ok)) process.exitCode = 1;
