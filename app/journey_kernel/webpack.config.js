const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const CopyWebpackPlugin = require('copy-webpack-plugin');

module.exports = (env, argv) => {
  const isDevelopment = argv.mode === 'development';

  const plugins = [
    new HtmlWebpackPlugin({
      template: './static/index.html',
      filename: 'index.html'
    }),
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, "."),
      target: 'bundler',
      extraArgs: '--features wasm --no-default-features',
    }),
  ];

  // Only add CopyWebpackPlugin in development mode
  if (isDevelopment) {
    plugins.push(
      new CopyWebpackPlugin({
        patterns: [
          { 
            from: './static/token.json',
            to: './token.json'
          },
          {
            from: './journey_bitmap.bin',
            to: './journey_bitmap.bin'
          }
        ]
      })
    );
  }

  return {
    entry: './static/index.js',
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'bundle.js',
      assetModuleFilename: '[name][ext]',
      webassemblyModuleFilename: 'journey_kernel_bg.wasm',
    },
    experiments: {
      asyncWebAssembly: true,
    },
    module: {
      rules: [
        {
          test: /\.css$/i,
          use: ['style-loader', 'css-loader'],
        },
      ],
    },
    plugins,
    devServer: {
      static: './dist',
    },
  };
}; 