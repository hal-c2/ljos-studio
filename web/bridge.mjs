// The preview bridge's handshake (`preview.js`) in node, where the order of
// a late answer and the next request can be set by hand: the browser's
// event loop decides it in `web/browser.mjs`, so a race there is luck.
//
//   node web/bridge.mjs
//
// `workerAsk` runs in a worker thread, as Studio does in a page; `relay`
// runs here, its worker and frame stand-ins delivering what they are told
// when they are told.

import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { fileURLToPath } from "node:url";
import { TAG, VERSION, answerBuffer, relay, workerAsk } from "./preview.js";

if (!isMainThread) {
  const ask = workerAsk(workerData.buffer, (m) => parentPort.postMessage({ post: m }), { timeoutMs: 200 });
  for (const request of ["one", "two"]) {
    let out;
    try {
      out = { answer: ask(JSON.stringify(request)) };
    } catch (e) {
      out = { error: e.message };
    }
    parentPort.postMessage({ asked: request, ...out });
  }
} else {
  const buffer = answerBuffer(1 << 10);
  const thread = new Worker(fileURLToPath(import.meta.url), { workerData: { buffer } });
  const page = new EventTarget();
  const frameWindow = {};
  const toFrame = [];
  frameWindow.postMessage = (m) => toFrame.push(m);
  const fakeWorker = new EventTarget();
  relay({ worker: fakeWorker, frame: { contentWindow: frameWindow }, buffer, window: page });
  const deliver = (target, data, source) => {
    const event = new Event("message");
    Object.assign(event, { data, source });
    target.dispatchEvent(event);
  };
  const frameAnswers = (id, answer) => deliver(page, { [TAG]: VERSION, id, answer }, frameWindow);

  const results = [];
  const checks = [];
  const check = (what, ok, detail) => checks.push({ what, ok, detail });
  await new Promise((done) => {
    thread.on("message", (m) => {
      if (m.post) {
        if (m.post.id === 1) {
          deliver(fakeWorker, m.post); // reaches the frame, which is slow
        } else {
          // The frame's late answer to the first lands before the second
          // request is carried: the relay still waits on the first.
          frameAnswers(1, "late one");
          deliver(fakeWorker, m.post);
          frameAnswers(2, "two");
        }
      } else {
        results.push(m);
        if (results.length === 2) done();
      }
    });
  });
  await thread.terminate();
  check("a request the frame does not answer in time times out", /did not answer/.test(results[0].error ?? ""), results[0]);
  check("the next request gets its own answer, not the late one", results[1].answer === JSON.stringify("two"), results[1]);
  check("both requests reached the frame", toFrame.map((m) => m.id).join() === "1,2", toFrame);
  for (const { what, ok, detail } of checks) console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (checks.some((c) => !c.ok)) process.exitCode = 1;
}
