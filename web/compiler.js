// The Compiler a preview page calls: QML files in, ljos-web modules or
// diagnostics out. It is the one seam between a page and how QML becomes
// JavaScript (DESIGN.md S5, "The Compiler").
//
//   compile({ files, entry, aliases }) -> Promise<
//     { component, base? } | { diagnostics: [{ file?, line?, column?, message }] }>
//
// `files` maps a project path to its text, `entry` is the document to show
// and `aliases` (`[{ module, dir }]`) import a module from another directory,
// as `ljos run --alias` does; `base` is the address the compile's files are
// at, for a data file told of later. This one is a server's, running ljos-web's
// `qmlc`; `ljos-web/compiler`, which runs in the page, has the same shape.

// qmlc says where a problem is as `path:line:column: message`.
const PLACE = /^(.+?):(\d+):(\d+): (.*)$/;

export function diagnostics(text) {
  const found = [];
  for (const line of String(text).split("\n")) {
    const place = PLACE.exec(line.trim());
    if (place) found.push({ file: place[1], line: Number(place[2]), column: Number(place[3]), message: place[4] });
  }
  return found.length > 0 ? found : [{ message: String(text).trim() }];
}

// The server that served the page compiles: `POST endpoint` answers with
// the URL of the compiled entry, which the page imports.
export function serverCompiler({ endpoint = "/compile", fetch: get = globalThis.fetch, load = (url) => import(/* @vite-ignore */ url) } = {}) {
  return async ({ files, entry, aliases = [] }) => {
    const asked = await get(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ files, entry, aliases }),
    });
    const { url, base, error } = await asked.json();
    if (error !== undefined) return { diagnostics: diagnostics(error) };
    return { component: (await load(url)).default, base };
  };
}

// For tests: a "component" that shows the entry's text, or the
// diagnostics a test gives for a file whose text matches.
export function fakeCompiler({ broken = /\{\{/ } = {}) {
  return async ({ files, entry, aliases = [] }) => {
    const text = files[entry];
    if (text === undefined) return { diagnostics: [{ file: entry, message: "no such file" }] };
    if (broken.test(text)) return { diagnostics: [{ file: entry, line: 1, column: 1, message: "does not parse" }] };
    const shown = aliases.length > 0 ? `${text}\n(aliased: ${aliases.map((a) => a.module).join(", ")})` : text;
    return { component: shown };
  };
}
