// Studio's shell, compiled by ljos-web, in headless Chromium against the
// session in wasm (`mise run web:shell` builds it): a person's gestures go
// through `Studio.Workspace`'s face as tool calls, and what they did comes
// back as the shell shows it.
//
//   PLAYWRIGHT=… node web/shell.mjs <the built page's directory> [screenshot.png]
//
// Nothing is served from a network: Playwright answers the page from the
// directory.

import { readFileSync } from "node:fs";
import { extname, join } from "node:path";

const [dist, shot] = process.argv.slice(2);
const { chromium } = await import(process.env.PLAYWRIGHT ?? "playwright");
const ORIGIN = "https://shell.test";
const TYPES = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".wasm": "application/wasm" };

const browser = await chromium.launch({ headless: true });
// The size `shell/Window.qml` gives its window: on the web the window fills
// the page.
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const failures = [];
const said = [];
page.on("pageerror", (e) => failures.push(`page error: ${e.message}`));
page.on("console", (m) => m.type() === "error" && said.push(m.text()));
await page.route(`${ORIGIN}/**`, (route) => {
  const asked = new URL(route.request().url()).pathname;
  const path = asked === "/" ? "/index.html" : asked;
  return route.fulfill({
    status: 200,
    contentType: TYPES[extname(path)] ?? "application/octet-stream",
    body: readFileSync(join(dist, path)),
  });
});

const check = (what, ok, detail) => {
  console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failures.push(what);
};
const labelled = (name) => page.locator(`[aria-label="${name}"]`);
const code = () => page.evaluate(() => window.studio.text("Main.qml").text);
// The shell answers a gesture once the call and the view after it are in:
// wait for what it shows, not for a time.
const shows = (name) => labelled(name).first().waitFor({ timeout: 5000 }).then(() => true, () => false);

await page.goto(`${ORIGIN}/`);
await page.waitForFunction(() => window.mounted, null, { timeout: 10000 });

check("the documents panel lists the project's documents", await shows("Main.qml"));

await labelled("Main.qml").click();
check("opening a document lists its objects as layers", (await shows("Item root")) && (await shows("Rectangle card")));

await labelled("Rectangle card").click();
check("selecting a layer shows its bindings in the inspector", await shows("Bind color"));

// The inspector's row for `width`: a field that sets it on Enter. (The
// row for a new property has no room for its value under ljos-web's Basic
// style in a 300-pixel panel: DESIGN.md S5.)
const width = labelled("width").locator("input");
await width.fill("150");
await width.press("Enter");
await page.waitForFunction(() => window.studio.text("Main.qml").text.includes("width: 150"), null, { timeout: 5000 }).catch(() => {});
check("setting a property in the inspector writes it into the document", (await code()).includes("width: 150"), await code());
check("the window's status shows no refusal", (await labelled("Status").innerText()) === "", await labelled("Status").innerText());

await labelled("Undo").click();
await page.waitForFunction(() => window.studio.text("Main.qml").text.includes("width: 120"), null, { timeout: 5000 }).catch(() => {});
check("undo takes it back", (await code()).includes("width: 120") && !(await code()).includes("width: 150"), await code());

// The tokens panel: a role filtered into view, set from its field, is the
// `theme_set` call, written into the theme file beside the document.
await labelled("Filter tokens").locator("input").fill("Spacing.md");
const md = labelled("Spacing.md").locator("input");
await md.waitFor({ timeout: 5000 }).catch(() => {});
await md.fill("20");
await md.press("Enter");
const theme = () => page.evaluate(() => window.files.get("theme.json") ?? "");
await page.waitForFunction(() => (window.files.get("theme.json") ?? "").includes('"md": 20'), null, { timeout: 5000 }).catch(() => {});
check("setting a token in the tokens panel writes the theme file", (await theme()).includes('"md": 20'), await theme());
await page.waitForFunction(() => document.querySelector('[aria-label="Spacing.md"] input')?.value === "20", null, { timeout: 5000 }).catch(() => {});
check("the tokens panel shows the token's new value", (await md.inputValue()) === "20", await md.inputValue());

// A call that follows from an edit. This page has no preview, so `preview_state`
// is refused without one being asked to show anything; the shell's own
// follow-ons do not run here, so these call the workspace as they would.
const status = () => labelled("Status").innerText();
const call = async (how, tool, args) => {
  await page.evaluate(([how, tool, args]) => window.workspaces()[0][how](tool, args), [how, tool, args]);
  await page.waitForFunction(() => {
    const { $queue: queue } = window.workspaces()[0];
    return queue.jobs.length === 0 && !queue.busy;
  });
};
const followed = { node: { id: "root" }, show: "compact" };
await call("follow", "preview_state", followed);
const own = await status();
check("a refused follow-on says why on an empty line", own !== "", own);
await call("dispatch", "no_such_tool", {});
const gesture = await status();
check("a gesture's refusal replaces it", gesture.includes("no_such_tool"), gesture);
await call("follow", "preview_state", followed);
check("a refused follow-on leaves the refusal on the line", (await status()) === gesture, await status());

if (shot) await page.screenshot({ path: shot });
// Whether a row's text is a literal is the parser's answer here too.
await labelled("Rectangle card").click();
await width.fill("0x20");
await width.press("Enter");
await page.waitForFunction(() => window.studio.text("Main.qml").text.includes("width: 0x20"), null, { timeout: 5000 }).catch(() => {});
check("a literal in QML's own form is written as typed", (await code()).includes("width: 0x20"), await code());
await width.fill("cube");
await width.press("Enter");
await labelled("cube is not a literal: bind it as an expression").first().waitFor({ timeout: 5000 }).catch(() => {});
check("what is no literal is refused", !(await code()).includes("cube"), await code());

// The picker asks the runtime what a colour is, so the browser's runtime
// has to answer as the native one does.
await labelled("Rectangle card").click();
await labelled("Pick color").first().click();
const colour = labelled("Colour").locator("input");
const before = await code();
for (const wrong of ["gren", "#12345"]) {
  await colour.fill(wrong);
  await colour.press("Enter");
}
await colour.fill("#123");
await colour.press("Enter");
await page.waitForFunction(() => window.studio.text("Main.qml").text.includes("#123"), null, { timeout: 5000 }).catch(() => {});
const after = await code();
check("the picker writes a colour", after.includes('"#123"') && after !== before, after);
check("the picker writes nothing for what is not one", !after.includes("gren") && !after.includes("#12345"), after);

check("the page logged no errors", said.length === 0, said);
await browser.close();
if (failures.length > 0) {
  console.log(`\n${failures.length} failed: ${failures.join("; ")}`);
  process.exitCode = 1;
}
