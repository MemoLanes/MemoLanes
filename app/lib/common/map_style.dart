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

  static const List<MapStyle> all = [
    normal,
    satellite,
    hybrid,
  ];

  static const MapStyle normal = MapStyle(
    id: 'normal',
    url: 'https://tiles.openfreemap.org/styles/liberty',
    copyright:
        '[OpenFreeMap](https://openfreemap.org) [© OpenMapTiles](https://www.openmaptiles.org/) Data from [OpenStreetMap](https://www.openstreetmap.org/copyright)',
    fogOpacity: 0.5,
  );

  static const MapStyle satellite = MapStyle(
    id: 'satellite',
    url: 'mapbox://styles/mapbox/satellite-v9',
    copyright:
        '[© Mapbox](https://www.mapbox.com/about/maps) [© OpenStreetMap](https://www.openstreetmap.org/copyright/) [Improve this map](https://www.mapbox.com/contribute/)',
    fogOpacity: 0.5,
  );

  static const MapStyle hybrid = MapStyle(
    id: 'hybrid',
    url: 'mapbox://styles/mapbox/satellite-streets-v12',
    copyright:
        '[© Mapbox](https://www.mapbox.com/about/maps) [© OpenStreetMap](https://www.openstreetmap.org/copyright/) [Improve this map](https://www.mapbox.com/contribute/)',
    fogOpacity: 0.5,
  );

  static MapStyle findById(String? id) {
    return all.firstWhere(
      (s) => s.id == id,
      orElse: () => normal,
    );
  }
}
