---
name: frb-codegen
description: Handle MemoLanes Flutter Rust Bridge generated code. Use for Rust API changes, FRB-generated errors, or choosing between a fast Rust-only compile and full bridge generation. Never edit generated output manually.
---

# FRB code generation

Never edit generated output under `app/lib/src/rust/`,
`app/rust/src/frb_generated.*`, or `app/frb_generated.h`.

Choose one workflow from the repository root:

## Fast Rust-only check

Skip bridge generation when only checking that Rust compiles:

```sh
rm -f app/rust/src/frb_generated.rs
cargo check --manifest-path app/rust/Cargo.toml
```

`build.rs` creates a dummy bridge. This does not validate FRB or Flutter.

## Full bridge

Before Flutter work or final integration checks, generate the real bridge:

```sh
cd app
just frb-generate
```

Fix remaining problems in the Rust API, FRB configuration, or dependencies,
then regenerate.
