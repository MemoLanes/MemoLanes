import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';
import 'package:path/path.dart' as path;

void main(List<String> args) async {
  await build(args, (input, output) async {
    final cargoEnvironmentVariables = _cargoEnvironmentVariablesFor(input);

    await FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust',
      extraCargoEnvironmentVariables: cargoEnvironmentVariables,
    ).run(input: input, output: output);
  });
}

Map<String, String> _cargoEnvironmentVariablesFor(BuildInput input) {
  if (!input.config.buildCodeAssets) {
    return const <String, String>{};
  }

  if (input.config.code.targetOS == OS.android) {
    // TODO: Temporary workaround for native_toolchain_rust, which currently
    // hard-codes Android NDK API 35 instead of using
    // CodeConfig.android.targetNdkApi from Native Assets.
    // See https://github.com/GregoryConrad/native_toolchain_rust/issues/1.
    return _androidCargoEnvironmentVariables(input.config.code);
  }
  if (input.config.code.targetOS != OS.iOS) {
    return const <String, String>{};
  }

  // Use the same target as Flutter's generated framework Info.plist. The app
  // may require a newer iOS version than this library.
  return <String, String>{
    'IPHONEOS_DEPLOYMENT_TARGET': '${input.config.code.iOS.targetVersion}.0',
  };
}

Map<String, String> _androidCargoEnvironmentVariables(CodeConfig codeConfig) {
  final cCompiler = codeConfig.cCompiler;
  if (cCompiler == null) {
    throw UnsupportedError(
      'Native Assets did not provide an Android C compiler.',
    );
  }

  final (
    rustTargetTriple,
    ndkTargetTriple,
  ) = switch (codeConfig.targetArchitecture) {
    Architecture.arm64 => ('aarch64-linux-android', 'aarch64-linux-android'),
    Architecture.arm => ('armv7-linux-androideabi', 'armv7a-linux-androideabi'),
    Architecture.x64 => ('x86_64-linux-android', 'x86_64-linux-android'),
    final architecture => throw UnsupportedError(
      'Unsupported Android architecture: $architecture',
    ),
  };
  final apiTarget = codeConfig.android.targetNdkApi;
  final compilerDirectory = path.dirname(File.fromUri(cCompiler.compiler).path);
  final executableSuffix = Platform.isWindows ? '.cmd' : '';
  final clangPath = path.join(
    compilerDirectory,
    '$ndkTargetTriple$apiTarget-clang$executableSuffix',
  );
  final clangPpPath = path.join(
    compilerDirectory,
    '$ndkTargetTriple$apiTarget-clang++$executableSuffix',
  );

  for (final compilerPath in [clangPath, clangPpPath]) {
    if (!File(compilerPath).existsSync()) {
      throw StateError(
        'Cannot find the Android API $apiTarget compiler at $compilerPath.',
      );
    }
  }

  final targetEnvironmentName = rustTargetTriple.replaceAll('-', '_');
  return <String, String>{
    'CC_$targetEnvironmentName': clangPath,
    'CXX_$targetEnvironmentName': clangPpPath,
    'CARGO_TARGET_${targetEnvironmentName.toUpperCase()}_LINKER': clangPath,
  };
}
