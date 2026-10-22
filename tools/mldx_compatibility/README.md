# Journey metadata compatibility check

Run `bash tools/mldx_compatibility/check.sh` from the repository root. Rust and Git
network access are needed for the pinned dependencies on a first run. Set
`CARGO_TARGET_DIR` to reuse a build cache if desired.

The new core's integration tests produce three real MLDX files: absent metadata,
an empty metadata message, and known plus unknown string keys (including Unicode
and an empty value). A separate executable uses the complete, unmodified core at
`9641c0834102ccc70770dbea20600623a7514802`, before metadata existed, to read each
file, import it into SQLite, check geometry/time/raw-attachment state and export
it again. It does not substitute a reduced protobuf schema for the old importer.

The new-core tests also cover protobuf presence, enum accessors, database reopen,
editing the journey header and exporting/reimporting without losing unknown keys.
MLM version 1 and MLS version 2 remain unchanged. This verifies compatibility with
the pre-metadata MLS2 reader; readers that never supported MLS2 are outside this
baseline. Old apps can discard metadata. Import deduplication by ID/revision is
unchanged, so reimporting an identical revision does not repair discarded metadata.

The pinned legacy dependency and its lockfile must remain frozen when adding
future metadata keys; they are the regression baseline, not the current core.
