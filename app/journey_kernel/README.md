# WASM examples

1. build the package

```
wasm-pack build --target web --features wasm --no-default-features
```

2. run the tests natively (the test will generate the `journey_bitmap.bin` file)

```
cargo test
```

3. modify the `examples/journey_view.html` MAPBOX_ACCESS_TOKEN and open it with web server (live server, vscode live server, etc. we need the access file through relative path)

4. make sure links to `journey_bitmap.bin` in `examples/journey-sw.js` and `examples/journey_view.html` are correct.

5. open the `journey_view.html` in browser. You may see the following effect:

![journey_view](journey_view.png)
