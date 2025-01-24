# Journey Kernel Library


```
⚠️ When making changes to this library, remember to bump the ver`sion number in `Cargo.toml`. 
This ensures main rust project will properly rebuild the WASM module with the changes.
```

This library is used to render journey bitmaps in the app. It supports both native and WASM.

## JavaScript APIs

| API | Description | Parameters | Return Value |
|-----|-------------|------------|--------------|
| `window.updateLocationMarker(lng, lat, show, flyto)` | Updates the current location marker | `lng`: longitude (number)<br>`lat`: latitude (number)<br>`show`: visibility (boolean, default: true)<br>`flyto`: animate to location (boolean, default: false) | void |
| `window.getCurrentMapView()` | Gets current map position | none | `{lng: number, lat: number, zoom: number}` |
| `window.triggerJourneyUpdate()` | Manually triggers data update | none | Promise |

### URL Hash Parameters

Since flutter webview may not have a reliable way to inject JS *before* the page loads, we use URL hash parameters to optionally set the initial map position. The hash parameters can guarantee the location is set before the map is initialized.

All the parameters are optional.

```
https://example.com/#journey_id=XXXXX&lng=100.0&lat=30.0&zoom=19
```

| Parameter | Description | Type |
|-----------|-------------|------|
| `journey_id` | Initial journey id | string |
| `lng` | Initial longitude | number |
| `lat` | Initial latitude | number |
| `zoom` | Initial zoom level | number |

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