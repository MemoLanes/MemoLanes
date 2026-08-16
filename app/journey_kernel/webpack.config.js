const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const CopyWebpackPlugin = require("copy-webpack-plugin");

module.exports = (env, argv) => {
  const isDevelopment = argv.mode === "development";
  const isBenchmark = env?.benchmark === true || env?.benchmark === "true";
  const outputPath = path.resolve(
    __dirname,
    isBenchmark ? "dist-benchmark" : "dist",
  );

  const plugins = [
    new HtmlWebpackPlugin({
      template: "./static/index.html",
      filename: "index.html",
      chunks: ["main"], // Only include the main chunk
    }),
    // Android's WebView asset loader serves unknown extensions as text/plain,
    // so use .js for MapLibre's ESM worker and its imported sibling module.
    new CopyWebpackPlugin({
      patterns: [
        {
          from: path.resolve(
            __dirname,
            "node_modules/maplibre-gl/dist/maplibre-gl-worker.mjs",
          ),
          to: "maplibre-gl-worker.js",
          transform(content) {
            return content
              .toString()
              .replaceAll("maplibre-gl-shared.mjs", "maplibre-gl-shared.js");
          },
        },
        {
          from: path.resolve(
            __dirname,
            "node_modules/maplibre-gl/dist/maplibre-gl-shared.mjs",
          ),
          to: "maplibre-gl-shared.js",
        },
      ],
    }),
  ];
  if (!isBenchmark) {
    plugins.push(
      new HtmlWebpackPlugin({
        template: "./static/render_diagnostics_template.html",
        filename: "render_diagnostics.html",
        chunks: ["render_diagnostics"], // Only include the render_diagnostics chunk
      }),
    );
  }
  // Only add CopyWebpackPlugin in development mode
  if (isDevelopment) {
    plugins.push(
      new CopyWebpackPlugin({
        patterns: [
          {
            from: "./static/token.json",
            to: "./token.json",
          },
          {
            from: "./journey_bitmap.bin",
            to: "./journey_bitmap.bin",
          },
        ],
      }),
    );
  }
  if (isBenchmark) {
    plugins.push(
      new CopyWebpackPlugin({
        patterns: [
          {
            from: "./benchmark/blank-style.json",
            to: "./benchmark-style.json",
          },
        ],
      }),
    );
  }

  return {
    entry: isBenchmark
      ? { main: "./benchmark/benchmark-entry.js" }
      : {
          main: "./static/index.ts",
          render_diagnostics: "./static/render_diagnostics.ts",
        },
    output: {
      path: outputPath,
      filename: "[name].bundle.js",
      assetModuleFilename: "[name][ext]",
      // Remove or comment out this line to prevent webpack from expecting a separate WASM file
      // webassemblyModuleFilename: "journey_kernel_bg.wasm",
    },
    // experiments: {
    //   asyncWebAssembly: true,
    // },
    module: {
      rules: [
        {
          test: /\.tsx?$/,
          use: "ts-loader",
          exclude: /node_modules/,
        },
        {
          test: /\.css$/i,
          use: ["style-loader", "css-loader"],
        },
        {
          test: /\.wasm$/,
          type: "asset/inline",
        },
      ],
    },
    resolve: {
      extensions: [".tsx", ".ts", ".js"],
    },
    plugins,
    devServer: {
      static: outputPath,
      client: {
        overlay: {
          // Show errors in overlay, but filter out aborted fetch errors
          // which are normal during map panning/zooming
          errors: true,
          warnings: false,
          runtimeErrors: (error) => {
            // Suppress aborted fetch errors (common in map libraries)
            if (error && error.message) {
              const msg = error.message.toLowerCase();
              if (
                msg.includes("abort") ||
                msg.includes("cancelled") ||
                msg.includes("canceled")
              ) {
                return false;
              }
            }
            return true;
          },
        },
      },
    },
  };
};
