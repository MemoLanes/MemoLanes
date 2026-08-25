import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';
import 'package:path/path.dart' as path;

void main(List<String> args) async {
  await build(args, (input, output) async {
    final cargoEnvironmentVariables = await _cargoEnvironmentVariablesFor(
      input: input,
      output: output,
    );

    await FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust',
      extraCargoEnvironmentVariables: cargoEnvironmentVariables,
    ).run(input: input, output: output);
  });
}

Future<Map<String, String>> _cargoEnvironmentVariablesFor({
  required BuildInput input,
  required BuildOutputBuilder output,
}) async {
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

  // TODO: Temporary workaround for native_toolchain_rust, which currently does
  // not forward CodeConfig.iOS.targetVersion as IPHONEOS_DEPLOYMENT_TARGET.
  final projectFile = input.packageRoot.resolve(
    'ios/Runner.xcodeproj/project.pbxproj',
  );
  output.dependencies.add(projectFile);
  final deploymentTarget = await _readDeploymentTarget(projectFile);

  return <String, String>{
    'IPHONEOS_DEPLOYMENT_TARGET': deploymentTarget,
  };
}

Map<String, String> _androidCargoEnvironmentVariables(CodeConfig codeConfig) {
  final cCompiler = codeConfig.cCompiler;
  if (cCompiler == null) {
    throw UnsupportedError(
      'Native Assets did not provide an Android C compiler.',
    );
  }

  final (rustTargetTriple, ndkTargetTriple) =
      switch (codeConfig.targetArchitecture) {
    Architecture.arm64 => (
        'aarch64-linux-android',
        'aarch64-linux-android',
      ),
    Architecture.arm => (
        'armv7-linux-androideabi',
        'armv7a-linux-androideabi',
      ),
    Architecture.x64 => (
        'x86_64-linux-android',
        'x86_64-linux-android',
      ),
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

Future<String> _readDeploymentTarget(Uri projectFile) async {
  final file = File.fromUri(projectFile);
  if (!await file.exists()) {
    throw StateError(
      'Cannot find the iOS Xcode project at ${file.path}. '
      'Native Assets cannot determine IPHONEOS_DEPLOYMENT_TARGET.',
    );
  }

  final deploymentTargets = RegExp(
    r'IPHONEOS_DEPLOYMENT_TARGET\s*=\s*([0-9]+(?:\.[0-9]+){0,2})\s*;',
  )
      .allMatches(await file.readAsString())
      .map((match) => match.group(1)!)
      .toSet();

  if (deploymentTargets.isEmpty) {
    throw StateError(
      'No numeric IPHONEOS_DEPLOYMENT_TARGET was found in ${file.path}. '
      'Set it in the Xcode project before building Native Assets.',
    );
  }
  if (deploymentTargets.length > 1) {
    throw StateError(
      'All iOS targets must use the same IPHONEOS_DEPLOYMENT_TARGET for '
      'Native Assets. Found: ${deploymentTargets.join(', ')} in ${file.path}.',
    );
  }

  return deploymentTargets.single;
}
