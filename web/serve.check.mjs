// What a person does at `mise run web:serve`, in headless Chromium against the
// served URL (`mise run web:serve:check` builds the page): the page loads
// cross-origin isolated, a project opens from browser storage, an edit in the
// shell reaches storage and the preview, the project is there after a reload,
// a folder imports as a project, and what does not compile is said.
//
//   PLAYWRIGHT=… node web/serve.check.mjs <the assembled app's directory> [screenshot.png]

import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { start } from "./serve.mjs";

const [app, shot] = process.argv.slice(2);
const { chromium } = await import(process.env.PLAYWRIGHT ?? "playwright");
const { url, stop } = await start({ app });
console.log(`served at ${url}`);

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const page = await context.newPage();
const failures = [];
const said = [];
page.on("pageerror", (e) => failures.push(`page error: ${e.message}`));
page.on("console", (m) => m.type() === "error" && said.push(m.text()));

const check = (what, ok, detail) => {
  console.log(`${ok ? "✔" : "✘"} ${what}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failures.push(what);
};
const labelled = (name) => page.locator(`#app [aria-label="${name}"]`);
const shows = (name) => labelled(name).first().waitFor({ timeout: 10000 }).then(() => true, () => false);
const preview = page.frameLocator("#frame");
const state = () => page.locator("#state").innerText();
// The frame's root, drawn: what a person sees of the document.
const card = (color) => preview.locator(`#preview [style*="${color}"]`);
const settled = (n) => page.waitForFunction((want) => document.getElementById("state").textContent === `Up to date (${want})`, n, { timeout: 60000 }).then(() => true, () => false);

await page.goto(url);
// A first compile of Qt's controls is the page's slowest step.
await page.waitForFunction(() => window.mounted, null, { timeout: 60000 });
check("the page is cross-origin isolated", await page.evaluate(() => crossOriginIsolated));
check("the first project is kept in the browser's storage", /^(opfs|indexeddb):hello$/.test(await page.locator("#place").innerText()), await page.locator("#place").innerText());
check("the shell lists the project's documents", await shows("Main.qml"));

check("the preview draws the project", await settled(1), await state());
const widthOf = () => page.frames().find((f) => f !== page.mainFrame()).evaluate(() => {
  // The sample's one rectangle has the size the document gives it.
  const found = [...document.querySelectorAll("#preview *")].find((e) => getComputedStyle(e).backgroundColor === "rgb(70, 130, 180)");
  return found ? Math.round(found.getBoundingClientRect().width) : null;
});
check("the preview shows the document's rectangle", (await widthOf()) === 120, await widthOf());
const over = await page
  .waitForFunction(() => {
    const canvas = document.querySelector('#app [aria-label="Canvas"]')?.getBoundingClientRect();
    const shown = document.getElementById("preview").getBoundingClientRect();
    return canvas && ["left", "top", "width", "height"].every((k) => Math.abs(canvas[k] - shown[k]) < 1);
  }, null, { timeout: 10000 })
  .then(
    () => true,
    () => page.evaluate(() => ({ canvas: document.querySelector('#app [aria-label="Canvas"]')?.getBoundingClientRect().toJSON() ?? null, preview: document.getElementById("preview").getBoundingClientRect().toJSON() })),
  );
check("the preview is laid over the shell's Canvas panel", over === true, over);

await labelled("Main.qml").click();
await shows("Rectangle card");
await labelled("Rectangle card").click();
const width = labelled("width").locator("input");
await width.waitFor({ timeout: 10000 });
await width.fill("150");
await width.press("Enter");
await page.waitForFunction(() => window.files.get("Main.qml")?.includes("width: 150"), null, { timeout: 10000 }).catch(() => {});
check("an edit in the inspector reaches storage", await page.evaluate(() => window.files.get("Main.qml")?.includes("width: 150")));
check("the edit reaches the preview", (await settled(2)) && (await widthOf()) === 150, { state: await state(), width: await widthOf() });
if (shot) await page.screenshot({ path: shot });

// What does not compile leaves the last picture, and is said.
await page.evaluate(async () => {
  const { text, generation } = window.studio.text("Main.qml");
  await window.studio.replaceText("Main.qml", generation, 0, text.length, "import QtQuick\nItem { width: }\n");
});
const failed = await page.waitForFunction(() => document.getElementById("state").textContent === "Not updated", null, { timeout: 30000 }).then(() => true, () => false);
const problem = await page.locator("#problem").innerText();
check("what does not compile is said beside the preview", failed && problem.length > 0, { problem });
check("the last picture that worked stays", (await widthOf()) === 150, await widthOf());
await page.evaluate(async () => {
  const { text, generation } = window.studio.text("Main.qml");
  await window.studio.replaceText("Main.qml", generation, 0, text.length, "import QtQuick\n\nRectangle {\n    width: 77\n    height: 50\n    color: \"steelblue\"\n}\n");
});
await page.waitForFunction(() => document.getElementById("state").textContent.startsWith("Up to date"), null, { timeout: 30000 }).catch(() => {});
check("a mended document previews again", (await page.locator("#problem").innerText()) === "" && (await widthOf()) === 77, { width: await widthOf(), problem: await page.locator("#problem").innerText() });

await page.reload();
await page.waitForFunction(() => window.mounted, null, { timeout: 60000 });
check("the project is in storage after a reload", await page.evaluate(() => window.files.get("Main.qml")?.includes("width: 77")));
check("the reloaded page previews it", (await settled(1)) && (await widthOf()) === 77, await widthOf());

// A folder from disk becomes a project of its own.
const folder = mkdtempSync(join(tmpdir(), "studio-serve-"));
try {
  const project = join(folder, "imported");
  mkdirSync(join(project, "pages"), { recursive: true });
  writeFileSync(join(project, "Main.qml"), 'import QtQuick\n\nRectangle {\n    width: 60\n    height: 60\n    color: "steelblue"\n}\n');
  writeFileSync(join(project, "pages", "Other.qml"), "import QtQuick\n\nItem {}\n");
  writeFileSync(join(project, "picture.png"), "not text");
  await Promise.all([page.waitForURL(/project=imported/, { timeout: 30000 }), page.locator("#import").setInputFiles(project)]);
  await page.waitForFunction(() => window.mounted, null, { timeout: 60000 });
  const names = await page.evaluate(() => [...window.files.keys()].sort());
  check("a folder imports as a project, its text files only", JSON.stringify(names) === JSON.stringify(["Main.qml", "pages/Other.qml"]), names);
  check("the imported project previews", (await settled(1)) && (await widthOf()) === 60, await widthOf());
  await page.locator("#projects").selectOption("hello");
  await page.waitForURL(/project=hello/, { timeout: 30000 });
  await page.waitForFunction(() => window.mounted, null, { timeout: 60000 });
  check("the first project is still there", await page.evaluate(() => window.files.get("Main.qml")?.includes("width: 77")));
} finally {
  rmSync(folder, { recursive: true, force: true });
}

// Another site's page cannot use the compile endpoint.
const refused = await fetch(`${url}compile`, { method: "POST", headers: { origin: "https://elsewhere.example" }, body: "{}" });
check("the server refuses another site's page", refused.status === 403, refused.status);
const escape = await fetch(`${url}compile`, { method: "POST", body: JSON.stringify({ files: { "../../out.qml": "x" }, entry: "../../out.qml" }) });
check("the server refuses a path out of the project", escape.status === 400, escape.status);

check("the page logged no errors", said.length === 0, said);
await browser.close();
await stop();

// A checkout reached through a link: each project is still its own root, and
// what the compiler says still names files by the project's own paths.
const linkDir = mkdtempSync(join(tmpdir(), "studio-serve-link-"));
try {
  const link = join(linkDir, "app");
  symlinkSync(realpathSync(app), link);
  const linked = await start({ app: link });
  const compile = async (files) => {
    const answer = await fetch(`${linked.url}compile`, { method: "POST", body: JSON.stringify({ files, entry: "Main.qml" }) });
    return answer.json();
  };
  const main = "import QtQuick\nimport Parts\n\nPanel {}\n";
  const first = await compile({
    "Main.qml": main,
    // Found from a directory of its own only when the project is qmlc's root.
    "pages/Other.qml": main,
    "Parts/qmldir": "module Parts\nPanel 1.0 Panel.qml\n",
    "Parts/Panel.qml": "import QtQuick\n\nItem {}\n",
  });
  check("through a link, a project is its own root", first.url !== undefined, first);
  const second = await compile({ "Main.qml": main });
  check("through a link, the next project does not see that module", second.error?.includes("Panel") === true, second);
  check("through a link, an error names the project's own path", second.error?.includes(realpathSync(app)) === false, second);
  await linked.stop();
} finally {
  rmSync(linkDir, { recursive: true, force: true });
}
if (failures.length > 0) {
  console.log(`\n${failures.length} failed: ${failures.join("; ")}`);
  process.exitCode = 1;
}
