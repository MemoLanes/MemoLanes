import * as maplibregl from "maplibre-gl";
import init from "../../pkg/journey_kernel.js";
import wasm from "../../pkg/journey_kernel_bg.wasm";
import { checkPolarNavigation } from "./polar-navigation";
import { checkPolarCanvas } from "./polar-canvas";
import { checkPolarDisplay } from "./polar-display";
async function main() {
  await init({ module_or_path: wasm });
  maplibregl.setWorkerUrl(
    new URL("./maplibre-gl-worker.mjs", location.href).href,
  );
  const check = (condition, message) => {
    if (!condition) throw new Error(message);
  };
  const tests = [
    ...(await checkPolarNavigation(check)),
    ...(await checkPolarCanvas(check)),
    ...(await checkPolarDisplay(check)),
  ];
  window.renderingResult = { passed: tests.length, tests };
}
main().catch((error) =>
  setTimeout(() => {
    throw error;
  }),
);
