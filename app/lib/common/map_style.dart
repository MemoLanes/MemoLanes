/// Map base style: id, url, copyright, and fog opacity.
/// MMKV stores only [id]. Display labels are hardcoded in map_settings_page.
class MapStyle {
  const MapStyle({
    required this.id,
    required this.url,
    required this.copyright,
    required this.fogOpacity,
  });

  final String id;
  final String url;
  final String copyright;
  final double fogOpacity;

  // first one is the default.
  static const List<MapStyle> all = [
    MapStyle(
      id: 'openfreemap',
      url: 'https://tiles.openfreemap.org/styles/liberty',
      copyright:
          '[OpenFreeMap](https://openfreemap.org) [© OpenMapTiles](https://www.openmaptiles.org/) Data from [OpenStreetMap](https://www.openstreetmap.org/copyright)',
      fogOpacity: 0.5,
    ),
    MapStyle(
      id: 'maplibre',
      url: 'https://demotiles.maplibre.org/style.json',
      copyright: '[MapLibre](https://maplibre.org/)',
      fogOpacity: 0.5,
    ),
  ];

  static MapStyle findById(String? id) {
    return all.firstWhere(
      (s) => s.id == id,
      orElse: () => all[0],
    );
  }
}
