// What qmlc is shown of a project (`forQmlc`), run by `mise run web:serve:test`.

import assert from "node:assert/strict";
import { test } from "node:test";

import { forQmlc } from "./serve.mjs";

test("of two qmldirs of one module, the one at its import path is shown", () => {
  const shown = forQmlc({
    "App/Media/qmldir": "module App.Media\n",
    "mocks/App/Media/qmldir": "module App.Media\n",
  });
  assert.deepEqual(Object.keys(shown), ["App/Media/qmldir"]);
});

test("of two qmldirs of one module, neither at its import path, both are shown", () => {
  const files = {
    "one/qmldir": "module App.Media\n",
    "two/qmldir": "module App.Media\n",
  };
  assert.deepEqual(forQmlc(files), files);
});

test("a qmldir at the project's top declares a module of no directory", () => {
  const files = { qmldir: "module Top\n", "Top/qmldir": "module Top\n", "else/qmldir": "module Else\n" };
  assert.deepEqual(Object.keys(forQmlc(files)).sort(), ["Top/qmldir", "else/qmldir"]);
});

test("a qmldir that declares no module stays", () => {
  const files = { "parts/qmldir": "Panel 1.0 Panel.qml\n" };
  assert.deepEqual(forQmlc(files), files);
});

test("a block's line gives qmlc a type", () => {
  const shown = forQmlc({ "Parts/qmldir": "module Parts\n# block Card 1.0 Card.qml slots=body\n" });
  assert.equal(shown["Parts/qmldir"], "module Parts\n# block Card 1.0 Card.qml slots=body\nCard 1.0 Card.qml\n");
});

test("a block whose type is already declared is not declared again", () => {
  for (const line of ["Card 1.0 Card.qml", "singleton Card 1.0 Card.qml"]) {
    const text = `module Parts\n${line}\n# block Card 1.0 Card.qml\n`;
    assert.equal(forQmlc({ "Parts/qmldir": text })["Parts/qmldir"], text);
  }
});

test("a qmldir with no last newline still gets its block's line on a line of its own", () => {
  const shown = forQmlc({ "Parts/qmldir": "module Parts\n# block Card 1.0 Card.qml" });
  assert.equal(shown["Parts/qmldir"], "module Parts\n# block Card 1.0 Card.qml\nCard 1.0 Card.qml\n");
});

test("an alias's directory is shown at the module's path, in place of what was there", () => {
  const shown = forQmlc(
    {
      "App/Data/qmldir": "module App.Data\n",
      "App/Data/Real.qml": "real",
      "App/Data/deep/Kept.qml": "kept",
      "mocks/data/qmldir": "module App.Data\n",
      "mocks/data/Fake.qml": "fake",
      "mocks/data/deep/Other.qml": "other",
    },
    [{ module: "App.Data", dir: "mocks/data" }],
  );
  assert.equal(shown["App/Data/Fake.qml"], "fake");
  assert.equal(shown["App/Data/Real.qml"], undefined);
  assert.equal(shown["App/Data/deep/Kept.qml"], "kept");
  assert.equal(shown["App/Data/Other.qml"], undefined);
  assert.equal(shown["mocks/data/qmldir"], undefined);
});
