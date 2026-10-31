// A module that failed to load is loaded again by the next open: a network
// blip, or bytes that were wrong, must not refuse every later `openStudio`
// in the page until a reload.
//
//   node web/retry.mjs   (after `mise run web:build`)

import { readFileSync } from "node:fs";
import { Studio, openStudio } from "./studio.js";
import { memoryBackend } from "./store.js";

const wasm = readFileSync(new URL("./pkg/studio_web_bg.wasm", import.meta.url));
const main = "import QtQuick\n\nItem {\n    id: root\n    width: 10\n}\n";

// Not a module: what a failed fetch of one leaves.
const broken = await openStudio({ backend: memoryBackend({ "Main.qml": main }), wasm: new Uint8Array([0, 97, 115, 109]) }).then(
  () => null,
  (e) => e,
);
const opened = await openStudio({ backend: memoryBackend({ "Main.qml": main }), wasm }).then(
  (studio) => studio,
  (e) => e,
);
const checks = [
  ["an open whose module does not load rejects", broken instanceof Error, String(broken)],
  ["the next open loads it again and opens", opened instanceof Studio, String(opened)],
];
if (opened instanceof Studio) {
  const tree = await opened.call("document_tree", { doc: "Main.qml" });
  checks.push(["and its Studio answers", tree.ok?.nodes !== undefined, tree]);
  await opened.close();
}

for (const [what, ok, detail] of checks) console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
if (checks.some(([, ok]) => !ok)) process.exitCode = 1;
