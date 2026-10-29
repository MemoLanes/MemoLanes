import assert from "node:assert/strict";
import test from "node:test";

// Load the diagnostics entry without firing browser UI callbacks.
const previousWindow = globalThis.window;
const previousDocument = globalThis.document;
globalThis.window = {
  location: { hash: "" },
  addEventListener() {},
};
globalThis.document = { addEventListener() {} };
let detectWasmFeatures;
try {
  ({ detectWasmFeatures } = await import("./render_diagnostics.ts"));
} finally {
  if (previousWindow === undefined) delete globalThis.window;
  else globalThis.window = previousWindow;
  if (previousDocument === undefined) delete globalThis.document;
  else globalThis.document = previousDocument;
}

test("diagnostic feature probes validate on the supported Node runtime", () => {
  assert.deepEqual(detectWasmFeatures(), {
    referenceTypes: true,
    simd: true,
    threads: true,
    bulkMemory: true,
    multiValue: true,
  });
});
