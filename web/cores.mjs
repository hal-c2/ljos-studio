// Two Studios in one wasm instance, as a page or runner opens them
// (`studio.js` loads the module once): each hears its own session's
// writes and no other's.
//
//   node web/cores.mjs   (after `mise run web:build`)

import { readFileSync } from "node:fs";
import { openStudio } from "./studio.js";
import { memoryBackend } from "./store.js";

const wasm = readFileSync(new URL("./pkg/studio_web_bg.wasm", import.meta.url));

const main = "import QtQuick\n\nItem {\n    id: root\n    width: 10\n}\n";
const [first, second] = await Promise.all(
  [0, 1].map(() => openStudio({ backend: memoryBackend({ "Main.qml": main }), wasm })),
);
for (const studio of [first, second]) studio.watch(() => {});
const set = (studio, width) => studio.call("property_set", { doc: "Main.qml", node: { id: "root" }, name: "width", value: width });

await set(second, 20);
await new Promise((resolve) => setTimeout(resolve));
const afterSecond = [first.writes, second.writes];
await set(first, 30);
await new Promise((resolve) => setTimeout(resolve));
const afterFirst = [first.writes, second.writes];

const checks = [
  ["the second Studio hears its own write", afterSecond[1] > 0, afterSecond],
  ["the first does not hear the second's", afterSecond[0] === 0, afterSecond],
  ["the first hears its own, and the second does not", afterFirst[0] > 0 && afterFirst[1] === afterSecond[1], afterFirst],
];
for (const [what, ok, detail] of checks) console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
if (checks.some(([, ok]) => !ok)) process.exitCode = 1;
