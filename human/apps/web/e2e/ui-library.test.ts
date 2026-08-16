import assert from "node:assert/strict";
import test from "node:test";

import { human_design_tokens } from "../src/design/tokens.ts";

test("the owner-supplied LayerX UI package is the visual contract", () => {
  const contract = human_design_tokens();
  assert.equal(contract.package, "@layerx/ui");
  assert.equal(contract.stylesheet, "@layerx/ui/styles.css");
  assert.ok(contract.styleFeatures.includes("borders"));
  assert.ok(contract.styleFeatures.includes("shadows"));
  assert.ok(contract.tokens.includes("--border"));
  assert.ok(contract.tokens.includes("--shadow-overlay"));
});
