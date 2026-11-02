// What qmlc is shown of a project (`forQmlc`), run by `mise run web:serve:test`.

import assert from "node:assert/strict";
import { test } from "node:test";

import { forQmlc } from "./serve.mjs";

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
