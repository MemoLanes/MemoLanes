import 'dart:io';

import 'package:flutter/services.dart' show rootBundle;
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as api;

/// Activating a worldview: the UI's only geo call.
///
/// Native Rust can't read Flutter's bundled assets, so the chosen worldview's
/// `geo_data_<id>.bin` is materialized (copied) into `<support>/geo` — the
/// backend's configured geo dir — before activation. Only the selected
/// worldview is copied, not all of them. The copy is re-done when the app's
/// build number changes, so an app update that ships new bins refreshes them.
class GeoService {
  static Future<void> setGeo(String worldviewId) async {
    final geoDir = p.join((await getApplicationSupportDirectory()).path, 'geo');
    await Directory(geoDir).create(recursive: true);

    final dst = File(p.join(geoDir, 'geo_data_$worldviewId.bin'));
    final stamp = File('${dst.path}.v');
    final build = (await PackageInfo.fromPlatform()).buildNumber;
    final have = await stamp.exists() ? await stamp.readAsString() : null;
    if (!await dst.exists() || have != build) {
      final data = await rootBundle.load('assets/geo/geo_data_$worldviewId.bin');
      await dst.writeAsBytes(data.buffer.asUint8List(), flush: true);
      await stamp.writeAsString(build, flush: true);
    }

    await api.setGeo(worldview: worldviewId);
  }
}
