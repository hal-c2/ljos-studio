// The JavaScript side of `crates/studio_web/src/protocol.rs`: how Studio
// asks a preview page and how the page's answer gets back.
//
// Studio asks synchronously (a tool call is one step of the session) and a
// page answers by message. DECISION: Studio runs in a dedicated worker and
// waits on a shared buffer while the page thread carries the request to the
// preview frame and writes the answer back. The page must be cross-origin
// isolated (COOP `same-origin`, COEP `require-corp`) for the buffer to be
// shared; the wait is bounded, so a hung preview times out instead of
// hanging Studio, and nothing polls while it waits.

export const TAG = "ljos-preview";
export const VERSION = 1;

const ANSWERED = 1;
const GONE = 2;
// State, length and the id of the request answered (Int32 each). The id
// is written last and is what the worker waits on: a late answer to a
// request that timed out can land after the worker has asked again, and
// it is not the new request's.
const HEADER = 12;

// A buffer for one answer at a time; a screenshot is the largest, base64.
export function answerBuffer(bytes = 32 << 20) {
  return new SharedArrayBuffer(HEADER + bytes);
}

// In the worker: the `ask(requestJson) -> answerJson` StudioCore is given.
// `post` sends to the page thread (the worker's `postMessage`).
export function workerAsk(buffer, post, { timeoutMs = 10_000 } = {}) {
  const header = new Int32Array(buffer, 0, 3);
  const body = new Uint8Array(buffer, HEADER);
  let next = 0;
  return (requestJson) => {
    const id = ++next;
    post({ [TAG]: VERSION, id, request: JSON.parse(requestJson) });
    const until = performance.now() + timeoutMs;
    for (let seen = Atomics.load(header, 2); seen !== id; seen = Atomics.load(header, 2)) {
      const left = until - performance.now();
      if (left <= 0 || Atomics.wait(header, 2, seen, left) === "timed-out") {
        throw new Error(`the preview did not answer in ${timeoutMs} ms`);
      }
    }
    const state = Atomics.load(header, 0);
    const text = new TextDecoder().decode(body.slice(0, Atomics.load(header, 1)));
    if (state === GONE) throw new Error(`gone: ${text}`);
    return text;
  };
}

// On the page thread: carries the worker's requests to `frame` and the
// frame's answers back into `buffer`. Returns `stop()`, for when the frame
// is removed: a request after that is answered "gone".
export function relay({ worker, frame, buffer, window: w = globalThis }) {
  const header = new Int32Array(buffer, 0, 3);
  const body = new Uint8Array(buffer, HEADER);
  let waiting = null;
  let gone = null;
  const answer = (state, text, id = waiting) => {
    const bytes = new TextEncoder().encode(text);
    if (bytes.length > body.length) {
      return answer(ANSWERED, JSON.stringify({ answer: "refused", code: "too_large", message: `${bytes.length} bytes do not fit in the answer buffer` }), id);
    }
    body.set(bytes);
    Atomics.store(header, 1, bytes.length);
    Atomics.store(header, 0, state);
    Atomics.store(header, 2, id);
    Atomics.notify(header, 2);
    waiting = null;
  };
  const fromWorker = (event) => {
    const message = event.data;
    if (message?.[TAG] !== VERSION) return;
    if (gone !== null) return answer(GONE, gone, message.id);
    waiting = message.id;
    frame.contentWindow.postMessage(message, "*");
  };
  const fromFrame = (event) => {
    const message = event.data;
    if (event.source !== frame.contentWindow || message?.[TAG] !== VERSION) return;
    if (message.id !== waiting) return; // an answer to a request that timed out
    answer(ANSWERED, JSON.stringify(message.answer));
  };
  worker.addEventListener("message", fromWorker);
  w.addEventListener("message", fromFrame);
  return function stop(reason = "the preview frame was removed") {
    gone = reason;
    w.removeEventListener("message", fromFrame);
    if (waiting !== null) answer(GONE, gone);
  };
}
