# Journey rendering benchmark

This benchmark starts the production-mode journey kernel bundle and a Rust map
server backed by the fixed `tests/data/fow_3.zip` fixture. It then runs three
fixed-camera scenarios in Chromium:

- pan at zoom 8
- zoom from 10 to 14 and back
- pan at zoom 14

Install the benchmark browser once:

```sh
yarn benchmark:install
```

Run the benchmark from `app/journey_kernel`:

```sh
yarn benchmark
```

The JSON report is printed to stdout and written to
`benchmark/results/latest.json`. Timestamped reports are stored beside it and
ignored by Git. There are no pass/fail thresholds; compare reports from the
same machine and environment manually.

The default viewport is 390x844 at DPR 2. It can be changed with
`BENCHMARK_WIDTH`, `BENCHMARK_HEIGHT`, and `BENCHMARK_DPR`. Set
`BENCHMARK_HEADLESS=false` to show Chromium, or `BENCHMARK_STATIC_PORT` to use a
different local web server port.

Each scenario reports animation-frame and MapLibre render-frame FPS, p50/p95/p99
frame time, the ratio of intervals over 20 ms and 33.3 ms, and the longest
interval. Browser, viewport, DPR, CPU concurrency, and GPU metadata are included
so reports from different environments are easy to identify.

Benchmark code is built only through the `benchmark:serve` script's separate
webpack entry and output directory. The normal `yarn build` entry graph does not
reference it.
