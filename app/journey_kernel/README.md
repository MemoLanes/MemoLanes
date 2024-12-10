# WASM examples

1. build the package

```
wasm-pack build --target web --features wasm --no-default-features
```

2. run the tests natively (the test will generate the `journey_bitmap.bin` file)

```
cargo test
```

3. check the `setup_token.py` has been executed again, and there is a `token.json` in the `examples` folder.

4. make sure links to `journey_bitmap.bin` in `examples/journey-view.html` are correct.

5. open the `journey-view.html` in browser. You may see the following effect:

You will see effect like this (the look may be different since we have updated since the screenshot):

![journey_view](journey_view.png)
