// A site from a Studio web export: the entry its manifest names, mounted.
// `mise run export:web:build` puts this beside the bundle's files (`site/`),
// with `node_modules` linked to an ljos-web checkout's.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import Entry from "ljos-site:entry";

mount(Entry, document.getElementById("app"));
