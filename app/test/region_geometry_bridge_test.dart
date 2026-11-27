import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement;
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';

Future<Uint8List> _load(String path) async =>
    Uint8List.sublistView(await rootBundle.load(path));

Matcher _throwsAssetError(String assetName) => throwsA(
  isA<AnyhowException>().having(
    (error) => error.toString(),
    'message',
    contains(assetName),
  ),
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final root = await Directory.systemTemp.createTemp(
      'region_geometry_bridge',
    );
    addTearDown(() => root.delete(recursive: true));
    Future<String> createDir(String name) async =>
        (await Directory('${root.path}/$name').create()).path;

    final libraryName = switch (Platform.operatingSystem) {
      'macos' => 'libmemolanes_core.dylib',
      'linux' => 'libmemolanes_core.so',
      'windows' => 'memolanes_core.dll',
      final os => throw UnsupportedError('Unsupported Flutter test host: $os'),
    };
    // Flutter test places the build hook's library outside Cargo's target directory.
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(
        'build/native_assets/${Platform.operatingSystem}/$libraryName',
      ),
    );
    addTearDown(RustLib.dispose);
    await api.init(
      tempDir: await createDir('temp'),
      docDir: await createDir('doc'),
      supportDir: await createDir('support'),
      systemCacheDir: await createDir('cache'),
    );
  });

  test(
    'a failed raster asset read is reported without a bridge panic',
    () async {
      const missingAsset = 'assets/geo/missing_geo_data_chn.bin';
      await expectLater(
        achievement.activateGeoData(
          worldview: achievement.Worldview.chn,
          loadAsset: () => _load(missingAsset),
        ),
        _throwsAssetError(missingAsset),
      );
    },
  );

  test('a failed display asset read is reported and can be retried', () async {
    await achievement.activateGeoData(
      worldview: achievement.Worldview.iso,
      loadAsset: () => _load('assets/geo/geo_data_iso.bin'),
    );
    final indonesia = achievement.GeoEntityId(field0: 1);
    const missingAsset = 'assets/geo/missing_region_display.bin';
    await expectLater(
      achievement.regionGeometry(
        entityId: indonesia,
        loadAsset: () => _load(missingAsset),
      ),
      _throwsAssetError(missingAsset),
    );

    final geometry = await achievement.regionGeometry(
      entityId: indonesia,
      loadAsset: () => _load('assets/geo/region_display.bin'),
    );
    expect(geometry?.entityId, indonesia);
  });
}
