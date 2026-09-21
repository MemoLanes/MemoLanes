---
name: webview-bundle
description: Build MemoLanes' Journey Kernel WebView bundle and copy it into Flutter assets. Use after editing app/journey_kernel/static or WASM code, when map_webview assets are missing, or when WebView changes do not appear in the app.
---

# Journey Kernel WebView bundle

The Flutter app loads `app/assets/map_webview/index.html` and its bundled
JavaScript. This directory is git-ignored, so source edits in
`app/journey_kernel/` do not reach the app until the bundle is rebuilt and
copied.

From `app/`, run:

```sh
just journey-kernel-build
just copy-webview-assets
```

On a fresh checkout, install Journey Kernel Node dependencies with `yarn
install` in `app/journey_kernel/` and ensure `wasm-pack` is available. The
repository's `just pre-build` workflow includes the build and copy steps along
with Geo and Flutter Rust Bridge generation.

For TypeScript validation, build first to create the ignored `pkg/` WASM
module, then run `yarn check-types` in `app/journey_kernel/`. For tests, run
`yarn test` there. Do not edit generated `pkg/`, `dist/`, or
`app/assets/map_webview/` files by hand.
