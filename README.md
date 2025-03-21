# MemoLanes

## Dev Env Setup
There are two main components of this app: 1. A rust library for the core logic; 2. a flutter project for the app itself. If you are only going to touch the rust core library then only 1 is required. However, working on the flutter project requires a full setup.

### 1. Rust Core Library Setup
1. Install [Rust](https://www.rust-lang.org/tools/install).
2. Install [protobuf](https://grpc.io/docs/protoc-installation/).
3. Install `just`. You can do it with `cargo install just` or use other package manager mentioned [here](https://just.systems/man/en/packages.html).
4. Go to `/app/rust` folder.
5. (If you are downgrading from a full setup, delete `src/frb_generated.rs` if it exists).
6. Run relevant cargo command to make sure the setup is completed (e.g. `cargo check`, `cargo test`).

### 3. Fultter Setup
1. Install [Flutter](https://docs.flutter.dev/get-started/install).
2. Install [flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/quickstart). It is recommnad install the specific version that this project is using by `cargo install 'flutter_rust_bridge_codegen@2.8.0'`. The real version can be found using `just get-frb-version` or looking at `app/pubspec.yaml`.
3. Install [yarn](https://yarnpkg.com/getting-started/install).
4. Go to `/app` folder.
5. Create `.env` file and put the Mapbox token in it. An example can be found in `.env.example`.
6. Running pre-build via `just pre-build`. Note that this need to be reran every time rust api or journey kernel is updated.
7. Start the app via `flutter run`.
8. `just` provides many useful commnads, e.g. `just format`, `just check`, `just test`. Consider run those before opening/updating PRs.