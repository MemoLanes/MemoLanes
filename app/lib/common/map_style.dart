import 'package:memolanes/common/map_fog_style.dart';

/// Map base style: id, url, copyright, and per-fog-style opacity.
/// MMKV stores only [id]. Display labels are hardcoded in map_settings_page.
class MapStyle {
  const MapStyle({
    required this.id,
    required this.url,
    required this.copyright,
    required this.fogOpacityByStyle,
  });

  final String id;
  final String url;
  final String copyright;

  /// Overlay opacity keyed by fog style ID, with values in the inclusive range
  /// 0–1.
  final Map<MapFogStyle, double> fogOpacityByStyle;

  // first one is the default.
  static const List<MapStyle> all = [
    MapStyle(
      id: 'openfreemap',
      url: 'https://tiles.openfreemap.org/styles/liberty',
      copyright: '[OpenFreeMap](https://openfreemap.org) [© OpenMapTiles](https://www.openmaptiles.org/) Data from [OpenStreetMap](https://www.openstreetmap.org/copyright)',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.60},
    ),
    MapStyle(
      id: 'maplibre',
      url: 'https://demotiles.maplibre.org/style.json',
      copyright: '[MapLibre](https://maplibre.org/)',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.60},
    ),
  ];

  static MapStyle findById(String? id) {
    return all.firstWhere((s) => s.id == id, orElse: () => all[0]);
  }
}
