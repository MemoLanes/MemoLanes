# Examples

This directory contains example applications that demonstrate how to use the MemoLanes core library.

## Structure

- `shared/` - Shared modules used by multiple examples
  - `map_server.rs` - HTTP server implementation for serving map tiles and API endpoints
- `server.rs` - Example server that demonstrates dynamic map rendering
- `app.rs` - Example application that imports and displays MLDX files

## HTTP Dependencies

The HTTP server functionality has been moved to this examples directory and uses dev-dependencies to avoid bloating the main library. The following dependencies are only available when running examples:

- `actix` - Actor framework
- `actix-web` - Web framework for HTTP server
- `ctrlc` - Signal handling for graceful shutdown
- `pollster` - Async runtime utilities

## Running Examples

```bash
# Run the dynamic server example
cargo run --example server

# Run the app example with an MLDX file
cargo run --example app path/to/file.mldx
```

## Migration Notes

If you were previously using `MapServer` from the main library (`memolanes_core::renderer::MapServer`), you'll need to:

1. Move your code to an example or create a separate binary crate
2. Add the HTTP dependencies to your `Cargo.toml` dev-dependencies
3. Import from the shared module: `mod shared; use shared::MapServer;`
