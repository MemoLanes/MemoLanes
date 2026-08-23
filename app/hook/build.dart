import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

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
  if (!input.config.buildCodeAssets || input.config.code.targetOS != OS.iOS) {
    return const <String, String>{};
  }

  final projectFile = input.packageRoot.resolve(
    'ios/Runner.xcodeproj/project.pbxproj',
  );
  output.dependencies.add(projectFile);
  final deploymentTarget = await _readDeploymentTarget(projectFile);
  final iOSVersionFlag =
      input.config.code.iOS.targetSdk == IOSSdk.iPhoneSimulator
          ? '-mios-simulator-version-min=$deploymentTarget'
          : '-mios-version-min=$deploymentTarget';

  return <String, String>{
    'IPHONEOS_DEPLOYMENT_TARGET': deploymentTarget,
    'RUSTFLAGS': '-C link-arg=$iOSVersionFlag',
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
