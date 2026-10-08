import assert from "node:assert/strict";
import test from "node:test";

import { settleGenerationAdvance } from "./generation-policy.ts";

test("an unchanged response consumes an explicit generation advance", () => {
  assert.equal(settleGenerationAdvance(false, true, "unchanged"), false);
});

test("a failed response retries an explicit generation advance", () => {
  assert.equal(settleGenerationAdvance(false, true, "failed"), true);
});

test("a newer refresh survives settlement of the in-flight request", () => {
  assert.equal(settleGenerationAdvance(true, true, "committed"), true);
  assert.equal(settleGenerationAdvance(true, true, "unchanged"), true);
  assert.equal(settleGenerationAdvance(true, true, "failed"), true);
});

test("an ordinary request cannot create a pending generation advance", () => {
  assert.equal(settleGenerationAdvance(false, false, "committed"), false);
  assert.equal(settleGenerationAdvance(false, false, "unchanged"), false);
  assert.equal(settleGenerationAdvance(false, false, "failed"), false);
});
