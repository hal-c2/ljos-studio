// The frame the page's preview runs in: `page.js` with a compiler, which here
// is the server that served the page. Each compile is a directory of its own,
// so the browser, which keeps a module for good, imports all of it anew.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import { servePreview } from "./page.js";

const READY = "ljos-serve-ready";

servePreview({
  element: document.getElementById("preview"),
  mount,
  async compile(files, entry) {
    const asked = await fetch("/compile", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ files, entry }),
    });
    const { url, error } = await asked.json();
    if (error) throw new Error(error);
    return (await import(/* @vite-ignore */ url)).default;
  },
});
// The page may be listening already, or may ask once it is (`main.js`).
addEventListener("message", ({ source, data }) => {
  if (source === parent && data?.[READY]) parent.postMessage({ [READY]: true }, "*");
});
parent.postMessage({ [READY]: true }, "*");
