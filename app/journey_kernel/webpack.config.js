const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const CopyWebpackPlugin = require("copy-webpack-plugin");

module.exports = (env, argv) => {
  const isDevelopment = argv.mode === "development";

  const plugins = [
    new HtmlWebpackPlugin({
      template: "./static/index.html",
      filename: "index.html",
      chunks: ["main"], // Only include the main chunk
    }),
    new HtmlWebpackPlugin({
      template: "./static/render_diagnostics_template.html",
      filename: "render_diagnostics.html",
      chunks: ["render_diagnostics"], // Only include the render_diagnostics chunk
    }),
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, "."),
      extraArgs: "--target web --features wasm --no-default-features",
    }),
  ];
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

  return {
    entry: {
      main: "./static/index.js",
      render_diagnostics: "./static/render_diagnostics.js",
    },
    output: {
      path: path.resolve(__dirname, "dist"),
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
          use: 'ts-loader',
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
      extensions: ['.tsx', '.ts', '.js'],
    },
    plugins,
    devServer: {
      static: "./dist",
    },
  };
};
