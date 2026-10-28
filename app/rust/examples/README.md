# Examples

This directory contains example applications that demonstrate how to use the MemoLanes core library.

## Structure

- `shared/` - Shared modules used by multiple examples
  - `map_server.rs` - HTTP server implementation for serving map tiles and API endpoints
- `server.rs` - Serves three demo maps by default, or a single GPX file with `--gpx`
- `app.rs` - Example application that imports and displays MLDX files

## Running Examples

```bash
# Run the dynamic server example
cargo run --example server

# Serve a single GPX through the production tile endpoints
cargo run --example server -- --gpx ./tests/data/polar_complex.gpx

# Run the app example with an MLDX file
cargo run --example app path/to/file.mldx
```

The `--gpx` mode prints `MAP_ENDPOINT=<url>` for scripts to read after the server
is ready, as well as a map page URL for manual inspection. It runs only the
requested GPX dataset. Stop either mode with Ctrl+C.
