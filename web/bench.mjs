// Numbers for DESIGN.md S5: a tool call's latency in node on a 200-object
// document, and what storing its write adds.
//
//   node web/bench.mjs

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { openStudio } from "./studio.js";
import { memoryBackend } from "./store.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasm = readFileSync(join(here, "pkg/studio_web_bg.wasm"));

// The root and 199 children: 200 objects.
export function document(objects = 200) {
  const children = Array.from(
    { length: objects - 1 },
    (_, i) => `    Rectangle {\n        id: r${i}\n        width: ${i}\n        height: 10\n        color: "red"\n    }\n`,
  );
  return `import QtQuick\n\nItem {\n    id: root\n\n${children.join("\n")}}\n`;
}

async function main() {
  const percentile = (sorted, p) => sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];
  const summary = (ms) => {
    const sorted = [...ms].sort((a, b) => a - b);
    return { p50: +percentile(sorted, 50).toFixed(3), p95: +percentile(sorted, 95).toFixed(3), n: ms.length };
  };

  const backend = memoryBackend({ "Main.qml": document() });
  const studio = await openStudio({ backend, wasm });
  let objects = 0;
  for (let cursor; ; ) {
    const page = await studio.call("document_tree", cursor ? { doc: "Main.qml", cursor } : { doc: "Main.qml" });
    if (page.error) throw new Error(JSON.stringify(page));
    objects += page.ok.nodes.length;
    if (!(cursor = page.ok.next_cursor)) break;
  }
  if (objects !== 200) throw new Error(`${objects} objects, not 200`);

  const RUNS = 50;
  const call = [];
  for (let i = 0; i < RUNS; i++) {
    const t = performance.now();
    const answer = await studio.call("property_set", { doc: "Main.qml", node: { id: `r${(i * 37) % 199}` }, name: "width", value: 1000 + i });
    call.push(performance.now() - t);
    if (answer.error) throw new Error(JSON.stringify(answer));
  }

  // What storing adds: the same calls against a backend that times its writes.
  const writes = [];
  const timed = memoryBackend({ "Main.qml": document() });
  const write = timed.write;
  timed.write = async (path, text) => {
    const t = performance.now();
    await write(path, text);
    writes.push(performance.now() - t);
  };
  const second = await openStudio({ backend: timed, wasm });
  for (let i = 0; i < RUNS; i++) {
    await second.call("property_set", { doc: "Main.qml", node: { id: `r${i}` }, name: "height", value: 20 + i });
  }

  console.log(
    JSON.stringify(
      { document_bytes: document().length, objects: 200, property_set_ms: summary(call), memory_store_write_ms: summary(writes) },
      null,
      2,
    ),
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
