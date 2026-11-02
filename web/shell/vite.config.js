// Studio's shell (`shell/*.qml`) as an ljos-web page. `mise run web:shell`
// puts this next to the shell, the stand-ins and Studio's JavaScript, with
// `node_modules` linked to an ljos-web checkout's.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import qml from "ljos-web/vite";

const path = (relative) => fileURLToPath(new URL(relative, import.meta.url));
const escaped = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// Compiled QML asks for the stand-in of a native type (the file `qmlc` was
// shown); the page is given the type's face instead. ljos-web will ask its
// module for the type once it reads `ljostypes.json` (its W1); the face is
// the same either way.
const faces = {
  name: "studio-faces",
  enforce: "pre",
  resolveId(source) {
    if (source.endsWith("standins/Studio/Workspace.qml")) return path("./Workspace.js");
    return null;
  },
};

export default defineConfig({
  base: "./",
  plugins: [
    faces,
    qml({
      qmlc: process.env.QMLC ?? "qmlc",
      // A project `serve.mjs` compiles is its own root: qmlc finds modules
      // by every qmldir under its root, and the others there are not this
      // project's. Only files under the directory `serve.mjs` names are
      // projects; another path with `projects/<n>` in it is not.
      args: (file) => {
        const projects = process.env.STUDIO_PREVIEW_PROJECTS?.replaceAll("\\", "/");
        const project = projects && new RegExp(`^(${escaped(projects)}/\\d+)/`).exec(file.replaceAll("\\", "/"));
        return project ? ["--root", project[1]] : [];
      },
      // A previewed project's files say where things are written in them.
      inspect: (file) => {
        const projects = process.env.STUDIO_PREVIEW_PROJECTS?.replaceAll("\\", "/");
        return Boolean(projects) && file.replaceAll("\\", "/").startsWith(`${projects}/`);
      },
      standins: [path("./standins"), ...(process.env.LJOS_DESIGN ? [process.env.LJOS_DESIGN] : [])],
    }),
  ],
  resolve: { dedupe: ["solid-js", "@solidjs/web", "ljos-web"] },
  // One page and its runtime: nothing here is loaded on demand.
  build: { target: "esnext", chunkSizeWarningLimit: 4000 },
});
