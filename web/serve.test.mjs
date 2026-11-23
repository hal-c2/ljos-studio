// What qmlc is shown of a project (`forQmlc`), run by `mise run web:serve:test`.

import assert from "node:assert/strict";
import { test } from "node:test";

import { compiledFirst, entryForQmlc, forQmlc, saidOf } from "./serve.mjs";

test("with no alias, qmlc is shown the project as it is", () => {
  const files = { "App/Media/qmldir": "module App.Media\n", "mocks/App/Media/qmldir": "module App.Media\n", "Parts/qmldir": "module Parts\n# block Card 1.0 Card.qml\n" };
  assert.deepEqual(forQmlc(files), files);
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

test("a hidden entry is shown beside itself under a JavaScript name", () => {
  const shown = entryForQmlc({ "Blocks/.Card.preview.qml": "stand-in", "Blocks/Card.qml": "card" }, "Blocks/.Card.preview.qml");
  assert.equal(shown.entry, "Blocks/_Card_preview.qml");
  assert.deepEqual(shown.files, { "Blocks/_Card_preview.qml": "stand-in", "Blocks/Card.qml": "card" });
  assert.deepEqual(compiledFirst(shown.files).sort(), ["Blocks/Card.qml", "Blocks/_Card_preview.qml"]);
});

test("the name given never takes a file's place", () => {
  const shown = entryForQmlc({ ".Card.preview.qml": "stand-in", "_Card_preview.qml": "theirs" }, ".Card.preview.qml");
  assert.equal(shown.entry, "_Card_preview_.qml");
  assert.equal(shown.files["_Card_preview.qml"], "theirs");
});

test("an entry that is named as one is left as it is", () => {
  const files = { "main.qml": "app" };
  assert.deepEqual(entryForQmlc(files, "main.qml"), { files, entry: "main.qml" });
});

test("a hidden file that is not run is not compiled", () => {
  assert.deepEqual(compiledFirst({ "Main.qml": "app", "Blocks/.Card.preview.qml": "// Mine" }), ["Main.qml"]);
});

test("what the compiler says of a renamed entry is said of the project's file, and of no other", () => {
  const said = saidOf(
    "_Card_preview.qml:3:5: no such type\nOther/_Card_preview.qml:2:1: no such type\nin _Card_preview.qml.js",
    "_Card_preview.qml",
    ".Card.preview.qml",
  );
  assert.equal(said, ".Card.preview.qml:3:5: no such type\nOther/_Card_preview.qml:2:1: no such type\nin _Card_preview.qml.js");
  assert.equal(saidOf("Blocks/_Card_preview.qml:1:1: x", "Blocks/_Card_preview.qml", "Blocks/.Card.preview.qml"), "Blocks/.Card.preview.qml:1:1: x");
  assert.equal(saidOf("Main.qml:1:1: x", "Main.qml", "Main.qml"), "Main.qml:1:1: x");
});
