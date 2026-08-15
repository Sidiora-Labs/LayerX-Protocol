import assert from "node:assert/strict";
import test from "node:test";

import { formatCopy } from "../copy/format.ts";

test("the copy formatter supports ICU plural branches", () => {
  assert.equal(formatCopy("approval.count", { count: 0 }), "No approvals waiting");
  assert.equal(formatCopy("approval.count", { count: 1 }), "1 approval waiting");
  assert.equal(formatCopy("approval.count", { count: 3 }), "3 approvals waiting");
});

test("the copy formatter supports ICU selection branches", () => {
  assert.equal(formatCopy("movement.direction", { direction: "inbound" }), "Money in");
  assert.equal(formatCopy("movement.direction", { direction: "outbound" }), "Money out");
});
