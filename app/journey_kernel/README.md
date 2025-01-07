# Journey Kernel Library

This library is used to render journey bitmaps in the app. It supports both native and WASM.

## Web Development

The webpack project contains two special files that is handled differently in development and production:

- `journey_bitmap.bin`
- `token.json`

These two files are statically generated in dev mode, but will be hosted dynamically in production.

### Development

1. Run `setup_token.py` in the `app` folder to generate the `token.json` file.
2. Run `cargo test` in the `app/journey_kernel` folder to generate the `journey_bitmap.bin` file.
3. Run `yarn dev` in the `app/journey_kernel` folder to start the webpack dev server.

### Production

1. run `yarn build` to generate the static sites without the above two files. The output will be in the `dist` folder. (As an intermediate step, the wasm-pack will also generate wasm files in the `pkg` folder.)
2. run the following command in the `app/rust` folder:

```bash
cargo run --example server
```

The rust demo server is exactly the same as the one in the app. You may check the file at `app/rust/src/renderer/map_server.rs`.