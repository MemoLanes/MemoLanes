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
| `render`| Render method | 'canvas' |
| `debug`| debug panel | bool |

The page will listen to the hash change of `render` and `debug` and apply accordingly.

## Web Development

The webpack project contains two special files that is handled differently in development and production:

- `journey_bitmap.bin`
- `token.json`

These two files are statically generated in dev mode, but will be hosted dynamically in production.

### Development

1. Run `setup_token.py` in the `app` folder to generate the `token.json` file.
2. Run `cargo test` in the `app/journey_kernel` folder to generate the `journey_bitmap.bin` file.
3. Run `yarn dev` in the `app/journey_kernel` folder to start the webpack dev server.

#### Testing with Flutter App

To test the webpack dev server with the Flutter app (useful for hot-reload during web development):

```bash
flutter run --dart-define=DEV_SERVER=http://<your-ip>:8080
```

This will make the app load the map webview from the dev server instead of the bundled assets.

#### Testing with Rust Demo Server

The Rust demo server only handles dynamic requests (e.g., tile data). You need to host the static resources separately via `yarn dev`.

1. Start the webpack dev server: `yarn dev` in the `app/journey_kernel` folder.
2. Start the Rust demo server in the `app/rust` folder:

```bash
cargo run --example server
```

By default, the server assumes static resources are available at `http://localhost:8080`. You can override this with the `DEV_SERVER` environment variable:

```bash
DEV_SERVER=http://localhost:8080 cargo run --example server
```

The Rust demo server logic is the same as the one in the app. You may check the file at `app/rust/src/renderer/map_server.rs`.

### Production Build

To generate the production build for the Flutter app:

```bash
# In the app folder
just pre-build
```

This command will:
1. Run `yarn build` to generate the static sites (output in `dist` folder, wasm files in `pkg` folder)
2. Copy the assets to Flutter's `assets/map_webview` folder
3. Generate the FRB code