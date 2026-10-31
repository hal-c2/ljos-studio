// Two Studios in one wasm instance, as a page or runner opens them
// (`studio.js` loads the module once): each hears its own session's
// writes and no other's, and closing one lets go of it alone.
//
//   node web/cores.mjs   (after `mise run web:build`)

import { readFileSync } from "node:fs";
import { heldFunctions, openStudio } from "./studio.js";
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
// Closing: each holds `ask` and its watcher.
const held = () => heldFunctions();
const opened = (ask) => openStudio({ backend: memoryBackend({ "Main.qml": main }), wasm, ask });
const threw = (call) => {
  try {
    call();
    return null;
  } catch (e) {
    return e;
  }
};
await first.close();
await second.close();
checks.push(["with both closed none of their functions is held", held() === 0, held()]);
const asked = () => JSON.stringify({ error: "not asked here" });
const [one, two] = [await opened(asked), await opened(asked)];
const both = held();
await one.close();
checks.push(["closing one lets go of its ask and its watcher", held() === both - 2, [both, held()]]);
const closed = await one.call("document_tree", { doc: "Main.qml" });
checks.push(["a closed Studio answers a call with a refusal", closed.error?.details?.kind === "closed", closed]);
for (const [what, call] of [
  ["tools", () => one.tools()],
  ["view", () => one.view("Main.qml")],
  ["text", () => one.text("Main.qml")],
  ["stored", () => one.stored("Main.qml", "Item {}\n")],
]) {
  const e = threw(call);
  checks.push([`${what} on a closed Studio throws an error, not a trap`, /closed/.test(e?.message) && !(e instanceof WebAssembly.RuntimeError), String(e)]);
}
const again = await one.close().then(() => null, String);
checks.push(["closing it again does nothing", again === null && held() === both - 2, again]);
const stillSet = await set(two, 40);
const stillRead = two.view("Main.qml");
checks.push(["the other still works", !stillSet.error && stillRead.text.includes("width: 40"), stillSet]);
await two.close();
checks.push(["and is let go of in turn", held() === 0, held()]);

const bad = await openStudio({ backend: memoryBackend({}), caps: "nonsense", wasm, ask: asked }).then(() => null, String);
checks.push(["a Studio that could not open holds nothing", bad !== null && held() === 0, [bad, held()]]);

// What storage has yet to do is done before the session goes, and a
// refusal of it rejects the close, which closes all the same.
{
  const backend = memoryBackend({ "Main.qml": main });
  const studio = await openStudio({ backend, wasm });
  const write = backend.write;
  backend.write = async () => {
    throw new Error("over quota");
  };
  const failed = await set(studio, 50).then(() => null, (e) => e);
  checks.push(["the refused write refuses its call", failed?.refusal?.details?.kind === "not_stored", String(failed)]);
  backend.write = write;
  await studio.close();
  checks.push(["closing stores what was refused before", backend.files.get("Main.qml").includes("width: 50"), backend.files.get("Main.qml")]);

  const refusing = memoryBackend({ "Main.qml": main });
  const last = await openStudio({ backend: refusing, wasm });
  refusing.write = async () => {
    throw new Error("over quota");
  };
  await set(last, 60).catch(() => {});
  const rejected = await last.close().then(() => null, (e) => e);
  checks.push(["a close storage refuses rejects as a call does", rejected?.refusal?.details?.kind === "not_stored", String(rejected)]);
  checks.push(["and is closed", (await last.call("document_tree", { doc: "Main.qml" })).error?.details?.kind === "closed", null]);
}

for (const [what, ok, detail] of checks) console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
if (checks.some(([, ok]) => !ok)) process.exitCode = 1;
