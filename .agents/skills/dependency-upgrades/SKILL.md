---
name: dependency-upgrades
description: Upgrade and verify dependencies across MemoLanes' Flutter/Dart, Rust, TypeScript, Node, and WASM toolchains. Use for a general repository-wide dependency refresh, outdated-dependency audit, lockfile refresh, or coordinated toolchain upgrade. Do not use for a single named dependency unless the user requests the full workflow.
---

# Dependency upgrades

## Inventory

Cover every ecosystem unless the request narrows the scope:

- Flutter/Dart: `app/pubspec.yaml` and the local packages under `app/rust_builder/`.
- Rust: `app/rust`, `app/rust/geo_data_format`, `app/journey_kernel`, and `tools/geo_rasterizer`.
- Journey web/WASM: `app/journey_kernel/package.json`.
- Toolchains: Node, Flutter, Rust, and `wasm-pack` pins in `.github/workflows/app.yml` and `app/journey_kernel/.nvmrc`.

Check current stable releases and relevant breaking changes before editing.

## Upgrade

- Update each manifest with its lockfile and prefer stable compatible releases.
- Preserve git/path overrides unless an upstream replacement is explicitly verified.
- Treat `geo_data_format` serialization changes, especially `bincode`, as file-format migrations rather than routine upgrades.
- Keep local and CI toolchain pins aligned and assess the Node/Rust/WASM toolchain together.

## Regenerate and verify

After Rust changes, regenerate bindings. From `app/`, run:

```sh
just frb-generate
just check
just test
just journey-kernel-build
```

For Flutter plugin or native-toolchain changes, also run:

```sh
flutter build ios --simulator --no-codesign
flutter build apk --debug
```

Re-run outdated-dependency checks and report intentional holds with their compatibility reason.
