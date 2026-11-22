// A site from a Studio web export (`ljos-site.json`, DESIGN.md S6, the site
// contract): ljos-web's compiler and Vite plugin over the bundle's files.
// Studio's builder (`build.mjs`) runs it; ljos-web's own `buildSite` makes a
// page that cannot scroll as a document.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import qml from "ljos-web/vite";

const path = (relative) => fileURLToPath(new URL(relative, import.meta.url));
const site = path("./site");
const manifest = JSON.parse(readFileSync(`${site}/ljos-site.json`, "utf8"));

export default defineConfig({
  base: "./",
  plugins: [
    qml({
      qmlc: process.env.QMLC ?? "qmlc",
      // The project's own modules are found by their qmldirs below the
      // bundle, wherever its entry is.
      args: ["--root", site],
      // ljos-web has no QML of Ljos.Design: whoever runs the builder puts
      // Ljós's here.
      standins: [path("./standins")],
      // Ljós's controls, ljos-web's own copy of them, as Studio's preview
      // draws them, whatever Qt is installed: a site is built alike wherever
      // Studio runs.
      styles: "ljos",
    }),
  ],
  resolve: {
    alias: { "ljos-site:entry": `${site}/${manifest.entry}` },
    dedupe: ["solid-js", "@solidjs/web", "ljos-web"],
  },
  build: { target: "esnext", chunkSizeWarningLimit: 4000 },
});
