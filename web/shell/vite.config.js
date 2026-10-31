// Studio's shell (`shell/*.qml`) as an ljos-web page. `mise run web:shell`
// puts this next to the shell, the stand-ins and Studio's JavaScript, with
// `node_modules` linked to an ljos-web checkout's.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import qml from "ljos-web/vite";

const path = (relative) => fileURLToPath(new URL(relative, import.meta.url));

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
      standins: [path("./standins"), ...(process.env.LJOS_DESIGN ? [process.env.LJOS_DESIGN] : [])],
    }),
  ],
  resolve: { dedupe: ["solid-js", "@solidjs/web", "ljos-web"] },
  // One page and its runtime: nothing here is loaded on demand.
  build: { target: "esnext", chunkSizeWarningLimit: 4000 },
});
