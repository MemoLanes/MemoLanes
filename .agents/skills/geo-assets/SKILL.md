---
name: geo-assets
description: Generate MemoLanes' ignored geographic data and localized region-name assets. Use on a fresh checkout before Flutter work, when assets/geo is missing, or after Geo source, worldview, or rasterizer changes.
---

# Geo assets

The Flutter app expects `app/assets/geo/geo_data_*.bin` and
`app/assets/geo/region_names.<locale>.json`. These files are generated and
git-ignored; an empty directory or placeholder JSON is not a valid setup.

From `app/`, run `just rasterize-geo` when Geo assets are missing or Geo inputs
change. It invokes `just registry-gen` before producing all supported
worldviews and locale names. The first run can download the pinned Natural
Earth source and take longer than later runs. For a fresh checkout that also
needs the WebView bundle and Flutter Rust Bridge, the repository's full
`just pre-build` workflow includes this step.

Inspect `git diff -- tools/geo_rasterizer/geo_entity_registry` after generation.
The registry is tracked source state, unlike `app/assets/geo/`; changes to its
frozen IDs need review alongside the Geo source or worldview change. See
`tools/geo_rasterizer/README.md` for the registry and data format.
