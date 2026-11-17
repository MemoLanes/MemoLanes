import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

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
  if (!input.config.buildCodeAssets || input.config.code.targetOS != OS.iOS) {
    return const <String, String>{};
  }

  // Use the same target as Flutter's generated framework Info.plist. The app
  // may require a newer iOS version than this library.
  final version = input.config.code.iOS.deploymentVersion;
  return <String, String>{
    'IPHONEOS_DEPLOYMENT_TARGET':
        '${version.major}.${version.minor}.${version.patch}',
  };
}
