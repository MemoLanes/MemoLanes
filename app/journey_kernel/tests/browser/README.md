# Canvas projection regressions

Run from `app/journey_kernel` after installing dependencies and building the
WASM package with `yarn wasm:dev`:

```sh
yarn benchmark:install
BENCHMARK_HEADLESS=true yarn test:browser
```

The browser suite checks the production `MapController` with both hemispheres:
jump/ease/fly destinations, globe projection hysteresis, initial journey bounds,
asymmetric padding and restoration when leaving journey focus. The suite also
checks the configured map zoom limit.
Pending raster basemap requests also exercise gesture-style camera updates:
both poles remain reachable and projection hysteresis works before tiles load.

Set `RENDER_TEST_BROWSER=webkit` to use an installed Playwright WebKit browser.
`BENCHMARK_CHROMIUM_EXECUTABLE` and `BENCHMARK_CHROMIUM_ANGLE` can select a local
Chromium and ANGLE backend. Headless mode is opt-in.
The shared bundler serves only temporary local assets and removes them on exit.
The Canvas suite exercises binary tile decoding through the WASM package,
Canvas rasterization and actual browser pixels. It checks both poles, native
zoom track positions in flat and spherical projections (including close to the
Mercator boundary), repeated flat worlds, single-opacity seams at several zoom
levels, independent cap downloads, forced refreshes and in-flight disposal.
During blended projections and pitched views, track positions are compared with
MapLibre-rendered reference markers rather than `map.project()`, which omits the
projection blend. Frame-by-frame pixel checks sample a grid across the viewport
center during animated and instant globe/Mercator zoom transitions, jumps into
polar seams and caps, and the return to Mercator. Synthetic tiles isolate known
geographic marker positions; the Rust map-renderer tests separately cover a
complex polar GPX fixture.

The display suite runs `MapController.initialize()` with endpoint-validating
binary responses and delayed individual planes. It verifies polar-only startup,
Mercator/polar seam readiness, non-blocking hidden caps, explicit refresh
completion and navigation between ordinary and polar-only views. Pixel checks
also require fog coverage on the very first polar frame of production navigation.
Switching a polar-only view back to a flat projection must restore Mercator
requests after MapLibre migrates the camera.
