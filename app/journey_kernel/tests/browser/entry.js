import * as maplibregl from "maplibre-gl";
import init from "../../pkg/journey_kernel.js";
import wasm from "../../pkg/journey_kernel_bg.wasm";
import { checkPolarNavigation } from "./polar-navigation";
import { checkPolarCanvas } from "./polar-canvas";
import { checkPolarDisplay } from "./polar-display";
import { checkMapAttribution } from "./map-attribution";
import {
  checkAchievementRevealPixels,
  checkAchievementResizeBeforeFit,
  checkAchievementPolarResizeBeforeFit,
  startAchievementStalledBasemap,
  checkAchievementKeepsMapSymbols,
  checkAchievementOverlayPreservesCollidingBasemap,
  checkAchievementEmptyCustomStyle,
  checkAchievementContextRecovery,
  checkAchievementStyleRetryAfterWebGLRestore,
  checkAchievementBasemapReady,
  checkAchievementOfflineStyle,
  checkAchievementRegionOverlay,
  checkSharedExplorerHandoff,
  checkSharedExplorerColdLocalReady,
} from "./achievement-region-overlay";

async function main() {
  await init({ module_or_path: wasm });
  maplibregl.setWorkerUrl(
    new URL("./maplibre-gl-worker.mjs", location.href).href,
  );
  const check = (condition, message) => {
    if (!condition) throw new Error(message);
  };
  const { result: stalled } = await startAchievementStalledBasemap(check);
  stalled.catch((error) =>
    setTimeout(() => {
      throw error;
    }),
  );
  const tests = [
    await checkAchievementResizeBeforeFit(check),
    await checkAchievementPolarResizeBeforeFit(check),
    ...(await checkAchievementRevealPixels(check)),
    await checkAchievementKeepsMapSymbols(check),
    await checkAchievementOverlayPreservesCollidingBasemap(check),
    await checkAchievementEmptyCustomStyle(check),
    ...(await checkAchievementContextRecovery(check)),
    await checkAchievementStyleRetryAfterWebGLRestore(check),
    ...(await checkPolarNavigation(check)),
    ...(await checkPolarCanvas(check)),
    ...(await checkPolarDisplay(check)),
    await checkMapAttribution(check),
    await checkAchievementBasemapReady(check),
    await checkAchievementOfflineStyle(check, false),
    await checkAchievementOfflineStyle(check, true),
    await checkAchievementRegionOverlay(check),
    await checkSharedExplorerHandoff(check),
    await checkSharedExplorerColdLocalReady(check),
    await stalled,
  ];
  window.renderingResult = { passed: tests.length, tests };
}
main().catch((error) =>
  setTimeout(() => {
    throw error;
  }),
);
