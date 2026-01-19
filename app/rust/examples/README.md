# Examples

This directory contains example applications that demonstrate how to use the MemoLanes core library.

## Structure

- `shared/` - Shared modules used by multiple examples
  - `map_server.rs` - HTTP server implementation for serving map tiles and API endpoints
- `server.rs` - Example server that demonstrates dynamic map rendering
- `app.rs` - Example application that imports and displays MLDX files

## Running Examples

```bash
# Run the dynamic server example
cargo run --example server

# Run the app example with an MLDX file
cargo run --example app path/to/file.mldx
```
