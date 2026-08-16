import assert from "node:assert/strict";
import test from "node:test";

import { SHELL_PROFILES, human_test_harness } from "./harness.ts";

test("the browser harness refuses anything except an explicitly real service", () => {
  assert.throws(() => human_test_harness({}), /HUMAN_E2E_REAL_STACK/);
  assert.throws(
    () => human_test_harness({ HUMAN_E2E_REAL_STACK: "0", HUMAN_E2E_BASE_URL: "http://127.0.0.1:3000" }),
    /substitutes are not accepted/,
  );
});

test("the real-stack harness exposes canonical mobile and desktop profiles", () => {
  const harness = human_test_harness({
    HUMAN_E2E_REAL_STACK: "1",
    HUMAN_E2E_BASE_URL: "http://127.0.0.1:3000",
  });
  assert.equal(harness.realStack, true);
  assert.equal(SHELL_PROFILES.mobile.viewport.width, 390);
  assert.equal(SHELL_PROFILES.desktop.viewport.width, 1440);
  assert.equal(SHELL_PROFILES.mobile.hasTouch, true);
  assert.equal(SHELL_PROFILES.desktop.hasTouch, false);
});
