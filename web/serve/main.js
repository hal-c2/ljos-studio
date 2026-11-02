// Studio for a person, on a page `mise run web:serve` serves: projects kept
// in the browser's storage, the shell over the session in wasm, and a preview
// of the project beside it, compiled by the server that served the page.
//
// DECISION: the preview here is the page's, not the session's `preview_*`
// tools. Those wait on a host answering synchronously from a worker, and the
// shell's `Workspace` reads the session synchronously on the page's thread;
// the two come together when ljos-web has a compiler in the page and a face
// over the worker (DESIGN.md S5, requests). The frame still speaks the preview
// protocol (`page.js`), so that is a change of who asks.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import { openBackend } from "./store.js";
import { openStudio } from "./studio.js";
import { serve } from "./Workspace.js";
import { TAG, VERSION } from "./preview.js";
import Window from "./qml/Window.qml";
import sample from "./sample.txt?raw";

const READY = "ljos-serve-ready";
const element = (id) => document.getElementById(id);

// --- Projects: their names in localStorage, as IndexedDB's cannot be listed
// the way OPFS's can, their files in the backend of that name. ------------
const KNOWN = "ljos-studio:projects";
const known = () => JSON.parse(localStorage.getItem(KNOWN) ?? "[]");
const remember = (name) => localStorage.setItem(KNOWN, JSON.stringify([...new Set([...known(), name])]));
const fresh = (wanted) => {
  let name = wanted;
  for (let i = 2; known().includes(name); i++) name = `${wanted}-${i}`;
  return name;
};
const go = (name) => {
  remember(name);
  location.search = `?project=${encodeURIComponent(name)}`;
};

const started = async (name) => {
  const backend = await openBackend(name);
  if (Object.keys(await backend.load()).length === 0) await backend.write("Main.qml", sample);
  remember(name);
  return backend;
};

const name = new URLSearchParams(location.search).get("project") ?? known()[0] ?? "hello";
const backend = await started(name);

for (const each of known()) element("projects").add(new Option(each, each, false, each === name));
element("projects").onchange = (e) => go(e.target.value);
element("place").textContent = backend.place;
element("new").onclick = async () => {
  const wanted = prompt("The new project's name", "project");
  if (!wanted) return;
  const made = fresh(wanted);
  await started(made);
  go(made);
};

// What a project is made of in storage: text. A folder's pictures and the
// like are left out, as the session has no way to hold them.
const TEXT = /(\.(qml|js|mjs|json|conf|txt|ts)|\/qmldir)$/;
const SKIPPED = /(^|\/)(\.git|node_modules|target|build-[^/]*)\//;
element("import").onchange = async (e) => {
  const chosen = [...e.target.files]
    .map((file) => ({ file, path: file.webkitRelativePath.split("/").slice(1).join("/") }))
    .filter(({ path }) => TEXT.test(`/${path}`) && !SKIPPED.test(path));
  if (chosen.length === 0) {
    element("problem").textContent = "That folder has no QML or other text files to import.";
    return;
  }
  const made = fresh(e.target.files[0].webkitRelativePath.split("/")[0]);
  const into = await openBackend(made);
  for (const { file, path } of chosen) await into.write(path, await file.text());
  go(made);
};

// --- The session and the shell --------------------------------------------
// The files as storage has them, for the preview: written through, so the
// preview is told only of what storage already keeps.
const files = new Map(Object.entries(await backend.load()));
const kept = {
  place: backend.place,
  load: backend.load,
  async write(path, text) {
    await backend.write(path, text);
    files.set(path, text);
    refresh();
  },
  async remove(path) {
    await backend.remove(path);
    files.delete(path);
    refresh();
  },
};

const studio = await openStudio({ backend: kept, caps: "inspect,mutate,run" });
serve(studio);
const { object } = mount(Window, element("app"));

// --- The preview: the frame given the whole project after each write. A
// write during a compile is shown by the next one, so the frame always ends
// on the last. ------------------------------------------------------------
const frame = element("frame");
let next = 0;
const waiting = new Map();
let frameReady;
const ready = new Promise((resolve) => (frameReady = resolve));
addEventListener("message", ({ source, data }) => {
  if (source !== frame.contentWindow) return;
  if (data?.[READY]) return frameReady();
  if (data?.[TAG] !== VERSION) return;
  waiting.get(data.id)?.(data.answer);
  waiting.delete(data.id);
});
// The frame may have loaded while the session started, its word unheard, or
// may load after this, its ear not yet open: each says so to the other.
frame.contentWindow.postMessage({ [READY]: true }, "*");
const ask = (request) =>
  new Promise((resolve) => {
    const id = ++next;
    waiting.set(id, resolve);
    frame.contentWindow.postMessage({ [TAG]: VERSION, id, request }, "*");
  });

const entries = () => [...files.keys()].filter((path) => path.endsWith(".qml")).sort();
const fillEntries = () => {
  const select = element("entry");
  const was = select.value || (entries().includes("Main.qml") ? "Main.qml" : entries()[0]);
  select.replaceChildren(...entries().map((path) => new Option(path, path, false, path === was)));
};

let busy = false;
let again = false;
let generation = 0;
async function refresh() {
  if (busy) {
    again = true;
    return;
  }
  busy = true;
  await ready;
  do {
    again = false;
    fillEntries();
    const entry = element("entry").value;
    if (!entry) break;
    element("state").textContent = "Compiling…";
    const box = frame.getBoundingClientRect();
    const size = { width: Math.round(box.width), height: Math.round(box.height) };
    // An open lays the document out at this size, so it need not be asked again.
    told = wanted = JSON.stringify(size);
    const answer = await ask({
      call: "open",
      files: Object.fromEntries(files),
      entry,
      aliases: [],
      size,
    });
    // A failure leaves the last picture that worked on show, as a native
    // preview does when a reload fails.
    const failed = answer.answer !== "done";
    element("problem").textContent = failed ? (answer.message ?? JSON.stringify(answer)) : "";
    element("state").textContent = failed ? "Not updated" : `Up to date (${++generation})`;
  } while (again);
  busy = false;
  // The frame may have moved while it opened, when nothing was told a size:
  // the first open had no picture yet, and an open's size is the one it began with.
  place();
}
element("entry").onchange = refresh;
refresh();

// --- Where the preview is. DECISION: the frame is an element beside the
// shell's root, laid over the Canvas panel's box while the shell shows one
// (the column beside the shell otherwise). `PreviewPicture` has no face on
// the web: a native picture is a PNG the preview draws, and a page has none
// to give (ljos-web request 2) and needs none, as its frame draws itself.
// It follows the panel as the shell lays it out: observers, nothing polled.
const aside = element("preview");
// The shell's styles change far more often than the frame's size does, and a
// resize lays the document out again: only a new size is asked for, one at a
// time, and sizes that pass while one is asked for give way to the latest.
let wanted = null;
let told = null;
let telling = false;
const tellSize = async (size) => {
  wanted = JSON.stringify(size);
  if (telling) return;
  telling = true;
  while (wanted !== told) {
    told = wanted;
    await ask({ call: "resize", size: JSON.parse(told) });
  }
  telling = false;
};
let placing = false;
const place = () => {
  if (placing) return;
  placing = true;
  requestAnimationFrame(() => {
    placing = false;
    const canvas = element("app").querySelector('[aria-label="Canvas"]');
    const box = canvas?.getBoundingClientRect();
    const over = box !== undefined && box.width > 0 && box.height > 0;
    aside.classList.toggle("over", over);
    Object.assign(aside.style, over ? { left: `${box.left}px`, top: `${box.top}px`, width: `${box.width}px`, height: `${box.height}px` } : { left: "", top: "", width: "", height: "" });
    // The document is laid out at the size it now has.
    const frameBox = frame.getBoundingClientRect();
    if (generation > 0) tellSize({ width: Math.round(frameBox.width), height: Math.round(frameBox.height) });
    if (canvas !== watched) {
      if (watched) sized.unobserve(watched);
      if (canvas) sized.observe(canvas);
      watched = canvas;
    }
  });
};
let watched = null;
const sized = new ResizeObserver(place);
new MutationObserver(place).observe(element("app"), { childList: true, subtree: true, attributes: true, attributeFilter: ["style"] });
addEventListener("resize", place);
place();

// `files`: what storage was given, for a check to read.
Object.assign(window, { studio, files, shell: object, mounted: true });
