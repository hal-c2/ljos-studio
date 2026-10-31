// A site built from a Studio web export (`mise run export:web:check`), in
// headless Chromium: `features/web/site.feature`, step by step.
//
//   PLAYWRIGHT=… LJOS_DESIGN=… node web/site.mjs <the site's directory> <the project's directory>
//
// The site is answered from its directory below a path of its own, as a
// plain static server would, so it holds only if every address it asks for
// is its own. It prints what the site weighs and when it first painted.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { gzipSync } from "node:zlib";

const [site, project] = process.argv.slice(2);
const { chromium } = await import(process.env.PLAYWRIGHT ?? "playwright");
const ORIGIN = "https://site.test";
const BASE = "/apps/a-site/";
const TYPES = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".svg": "image/svg+xml", ".json": "application/json" };

const filesBelow = (dir) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? filesBelow(join(dir, e.name)) : e.name.startsWith(".") ? [] : [join(dir, e.name)],
  );
const files = filesBelow(site).map((f) => relative(site, f));

const failures = [];
const check = (what, ok, detail) => {
  console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failures.push(what);
};

// What Ljos.Design gives text in a light scheme of normal contrast at its
// own seed hue, read from its QML: the oracle is the module, not this page.
function designText() {
  const colors = readFileSync(join(process.env.LJOS_DESIGN, "Ljos/Design/Colors.qml"), "utf8");
  const seed = Number(/property real seedHue: (\d+)/.exec(colors)[1]);
  const light = [...colors.matchAll(/"light": \[([^\]]*)\]/g)].map((m) => JSON.parse(`[${m[1]}]`));
  const hex = light[Math.round((((seed % 360) + 360) % 360) / 30) % 12][13];
  const n = (at) => parseInt(hex.slice(at, at + 2), 16);
  return `rgb(${n(1)}, ${n(3)}, ${n(5)})`;
}

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 800, height: 600 }, colorScheme: "light" });
const said = [];
const asked = [];
page.on("pageerror", (e) => said.push(`page error: ${e.message}`));
page.on("console", (m) => (m.type() === "error" || m.type() === "warning") && said.push(m.text()));
page.on("request", (r) => asked.push(r.url()));
await page.route("**/*", (route) => {
  const url = new URL(route.request().url());
  const path = url.pathname === BASE ? "index.html" : url.pathname.slice(BASE.length);
  if (url.origin !== ORIGIN || !url.pathname.startsWith(BASE) || !files.includes(path)) {
    return route.fulfill({ status: 404, body: "" });
  }
  return route.fulfill({ status: 200, contentType: TYPES[extname(path)] ?? "application/octet-stream", body: readFileSync(join(site, path)) });
});

// When the site opened from a static server, below a path of its own
await page.goto(`${ORIGIN}${BASE}`);
const stopped = await page
  .getByText("Stopped", { exact: true })
  .waitFor({ timeout: 10000 })
  .then(() => true, () => false);
// The paint entry is queued after the frame it times, so it can be missing
// when the text is already there; a buffered observer waits for it.
const paint = await page.evaluate(async () => {
  const now = performance.now();
  const fcp = await new Promise((resolve) => {
    new PerformanceObserver((list, observer) => {
      const entry = list.getEntriesByName("first-contentful-paint")[0];
      if (entry) {
        observer.disconnect();
        resolve(entry.startTime);
      }
    }).observe({ type: "paint", buffered: true });
    setTimeout(() => resolve(null), 2000);
  });
  return { fcp, now };
});

// Then the page shows "Stopped"
check('the page shows "Stopped"', stopped, await page.evaluate(() => document.body.innerText));
// And the page draws the picture "images/logo.svg" 8 by 8 pixels
// ljos-web draws an Image as a box with the picture as its background; a
// small one Vite puts in the address itself, its quotes rewritten, so the
// two are compared as the SVG they parse to.
const logo = readFileSync(join(project, "images/logo.svg"), "utf8");
const pictures = await page.evaluate(() =>
  [...document.querySelectorAll("#app *")]
    .map((e) => ({ url: /^url\("(.*)"\)$/.exec(getComputedStyle(e).backgroundImage)?.[1], box: e.getBoundingClientRect() }))
    .filter((p) => p.url)
    .map((p) => ({ url: p.url.split("#")[0], w: p.box.width, h: p.box.height })),
);
const fetched = (url) =>
  url.startsWith("data:image/svg+xml,")
    ? decodeURIComponent(url.slice("data:image/svg+xml,".length))
    : url.startsWith(ORIGIN + BASE) && files.includes(url.slice((ORIGIN + BASE).length))
      ? readFileSync(join(site, url.slice((ORIGIN + BASE).length)), "utf8")
      : "";
const parsed = (text) =>
  page.evaluate((t) => new XMLSerializer().serializeToString(new DOMParser().parseFromString(t, "image/svg+xml")), text);
const want = await parsed(logo);
let drawn = false;
for (const p of pictures) {
  if (p.w === 8 && p.h === 8 && (await parsed(fetched(p.url))) === want) drawn = true;
}
check(
  'the page draws the picture "images/logo.svg" 8 by 8 pixels',
  drawn,
  pictures.map((p) => ({ ...p, url: p.url.slice(0, 80) })),
);
// And the text has the colour Ljos.Design gives text
const colour = await page.getByText("Stopped", { exact: true }).evaluate((e) => getComputedStyle(e).color).catch((e) => e.message);
check("the text has the colour Ljos.Design gives text", colour === designText(), { colour, design: designText() });
// And the page logged no errors
check("the page logged no errors", said.length === 0, said);

// Then every request the page made was for a file of the site
const strays = asked.filter((u) => !u.startsWith("data:")).filter((u) => {
  const url = new URL(u);
  const path = url.pathname === BASE ? "index.html" : url.pathname.slice(BASE.length);
  return url.origin !== ORIGIN || !url.pathname.startsWith(BASE) || !files.includes(path);
});
check("every request the page made was for a file of the site", strays.length === 0 && asked.length > 0, strays);
// And the site has an "index.html"
check('the site has an "index.html"', files.includes("index.html"), files);

// Then no file of the site names "mocks/" or ".studio"
const naming = files.filter((f) => {
  const text = readFileSync(join(site, f), "latin1");
  return f.includes("mocks/") || f.includes(".studio") || text.includes("mocks/") || text.includes(".studio");
});
check('no file of the site names "mocks/" or ".studio"', naming.length === 0, naming);
// And the page does not show "Playing", which only the mock module says
check('the page does not show "Playing"', !(await page.evaluate(() => document.body.innerText)).includes("Playing"));

await browser.close();

const bytes = files.reduce((n, f) => n + statSync(join(site, f)).size, 0);
const gzip = files.reduce((n, f) => n + gzipSync(readFileSync(join(site, f))).length, 0);
console.log(
  `site: ${files.length} files, ${bytes} bytes (${gzip} gzipped); first contentful paint ${paint.fcp === null ? "not reported" : `${paint.fcp.toFixed(1)} ms`}, entry's text shown by ${paint.now.toFixed(1)} ms`,
);
if (failures.length > 0) {
  console.error(`${failures.length} failed`);
  process.exit(1);
}
