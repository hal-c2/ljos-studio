// A site from a Studio web export (`ljos-site.json`, DESIGN.md S6, the site
// contract): ljos-web's compiler and Vite plugin over the bundle's files.
// Studio's interim builder until ljos-web has one of its own (request 30).
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
      standins: process.env.LJOS_DESIGN ? [process.env.LJOS_DESIGN] : [],
    }),
  ],
  resolve: {
    alias: { "ljos-site:entry": `${site}/${manifest.entry}` },
    dedupe: ["solid-js", "@solidjs/web", "ljos-web"],
  },
  build: { target: "esnext", chunkSizeWarningLimit: 4000 },
});
