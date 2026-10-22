#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
fixture_dir="$(mktemp -d "${TMPDIR:-/tmp}/mldx-compatibility.XXXXXX")"
trap 'rm -rf "$fixture_dir"' EXIT

# The core build supports clones without bundled map assets when CI is unset.
# Run from each crate directory because core tests use relative fixture paths.
(
  cd "$repo_root/app/rust"
  env -u CI TZ=UTC MLDX_COMPAT_FIXTURE_DIR="$fixture_dir" \
    cargo test --locked --test journey_metadata
)
(
  cd "$repo_root/tools/mldx_compatibility/legacy_reader"
  env -u CI TZ=UTC cargo run --locked -- "$fixture_dir"
)
