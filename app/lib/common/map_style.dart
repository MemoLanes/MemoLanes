/// Map base style: id, url, and copyright.
/// MMKV stores only [id]. Display labels are hardcoded in map_settings_page.
class MapStyle {
  const MapStyle({
    required this.id,
    required this.url,
    required this.copyright,
  });

  final String id;
  final String url;
  final String copyright;

  // first one is the default.
  static const List<MapStyle> all = [
    MapStyle(
      id: 'openfreemap',
      url: 'https://tiles.openfreemap.org/styles/liberty',
      copyright:
          '[OpenFreeMap](https://openfreemap.org) [© OpenMapTiles](https://www.openmaptiles.org/) Data from [OpenStreetMap](https://www.openstreetmap.org/copyright)',
    ),
    MapStyle(
      id: 'maplibre',
      url: 'https://demotiles.maplibre.org/style.json',
      copyright: '[MapLibre](https://maplibre.org/)',
    ),
  ];

  static MapStyle findById(String? id) {
    return all.firstWhere(
      (s) => s.id == id,
      orElse: () => all[0],
    );
  }
}
