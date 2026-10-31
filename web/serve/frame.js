// The frame the page's preview runs in: `page.js` with a Compiler, which here
// is the server that served the page. Each compile is a directory of its own,
// so the browser, which keeps a module for good, imports all of it anew.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import { inspect } from "ljos-web/inspect";
import { reload } from "ljos-web/reload";
import { changed, provide } from "ljos-web/files";
import { servePreview } from "./page.js";
import { serverCompiler } from "./compiler.js";

const READY = "ljos-serve-ready";

servePreview({
  element: document.getElementById("preview"),
  mount,
  reload,
  inspect,
  // `provide` tells readers only of a file it was given before.
  tell: (url, text, given) => {
    provide(url, text);
    if (!given) changed(url);
  },
  compile: serverCompiler(),
});
// The page may be listening already, or may ask once it is (`main.js`).
addEventListener("message", ({ source, data }) => {
  if (source === parent && data?.[READY]) parent.postMessage({ [READY]: true }, "*");
});
parent.postMessage({ [READY]: true }, "*");
