---
name: pre-build
description: Prepare MemoLanes generated code and assets for Flutter. Use on fresh checkouts, after Rust API, Geo/worldview, or WebView/WASM changes, or for missing/stale app/lib/src/rust/, frb_generated.*, app/assets/geo/, app/assets/map_webview/, or app/journey_kernel/pkg/. Also use when WebView changes do not appear in the app or FRB-generated code fails to compile.
---

# Build preparation

Run commands from `app/`. Use `just pre-build` for full preparation on a fresh
checkout. For an existing checkout, regenerate only missing or affected outputs:

| Change or missing output | Command |
| --- | --- |
| Rust API / FRB configuration; `lib/src/rust/`, `rust/src/frb_generated.*`, `frb_generated.h` | `just frb-generate` |
| Geo sources / worldview / `tools/geo_rasterizer`; `assets/geo/geo_data_*.bin` or `region_names.*.json` | `just rasterize-geo` |
| `journey_kernel/static/` or WASM; `journey_kernel/pkg/` or `assets/map_webview/` | `just journey-kernel-build`, then `just copy-webview-assets` |

Ordinary Flutter UI edits need no generation when these outputs are current.
On a fresh checkout, install Node dependencies with `yarn install` in
`app/journey_kernel/` and ensure `wasm-pack` and `flutter_rust_bridge_codegen`
are available.

- Fix generation problems in source or configuration; never hand-edit generated
  code or replace Geo assets with placeholders.
- WebView source changes reach Flutter only after the bundle is built and copied.
  Generate `journey_kernel/pkg/` before running `yarn check-types` there.
- Geo generation also runs `registry-gen`. Review changes under
  `tools/geo_rasterizer/geo_entity_registry/`, which is tracked source.
- A Rust-only check using `build.rs`'s dummy bridge does not validate Flutter.
  Regenerate the real bridge with `just frb-generate` before Flutter integration.
