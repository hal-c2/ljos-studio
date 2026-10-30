// Studio's shell on a page: the session in wasm over a project in memory
// (`window.studioFiles` before the page loads, or a sample), served to the
// shell's workspaces, then the window mounted.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import { memoryBackend } from "./store.js";
import { openStudio } from "./studio.js";
import { serve } from "./Workspace.js";
import Window from "./qml/Window.qml";
import sample from "./sample.txt?raw";

const backend = memoryBackend(window.studioFiles ?? { "Main.qml": sample });
const studio = await openStudio({ backend, caps: "inspect,mutate,run" });
serve(studio);
const { object } = mount(Window, document.getElementById("app"));
// `files`: what storage was given, for a check to read.
Object.assign(window, { studio, files: backend.files, shell: object, mounted: true });
