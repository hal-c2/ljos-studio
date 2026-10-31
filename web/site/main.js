// A site from a Studio web export: the entry its manifest names, mounted.
// `mise run export:web:build` puts this beside the bundle's files (`site/`),
// with `node_modules` linked to an ljos-web checkout's.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import Entry from "ljos-site:entry";

const { object } = mount(Entry, document.getElementById("app"));

// A page is the entry's window, and a window sizes the root it shows: a root's
// declared size is where it starts, and breakpoint states read what it became.
const fit = () => {
  object.width = window.innerWidth;
  object.height = window.innerHeight;
};
window.addEventListener("resize", fit);
fit();
