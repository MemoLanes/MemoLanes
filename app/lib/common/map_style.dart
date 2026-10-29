import 'package:memolanes/common/map_fog_style.dart';

enum MapStyleType { style, raster }

enum MapTileScheme { xyz, tms }

/// Map base style: id, url, copyright, and per-fog-style opacity.
/// The selected style is stored by [id]; custom configurations are separate.
class MapStyle {
  const MapStyle({
    required this.id,
    required this.url,
    required this.fogOpacityByStyle,
    this.copyright = '',
    this.customName,
    this.type = MapStyleType.style,
    this.tileSize = 256,
    this.scheme = MapTileScheme.xyz,
  });

  final String id;
  final String url;

  /// Fixed attribution for built-in basemaps; custom maps use source metadata.
  final String copyright;
  final String? customName;
  final MapStyleType type;
  final int tileSize;
  final MapTileScheme scheme;

  bool get isCustom => customName != null;

  /// MapLibre accepts either a style URL or a complete style object.
  Object get mapLibreStyle => type == MapStyleType.style
      ? url
      : {
          'version': 8,
          'sources': {
            'basemap': {
              'type': 'raster',
              'tiles': [url],
              'tileSize': tileSize,
              'scheme': scheme.name,
            },
          },
          'layers': [
            {'id': 'basemap', 'type': 'raster', 'source': 'basemap'},
          ],
        };

  bool hasSameRenderer(MapStyle other) =>
      id == other.id &&
      url == other.url &&
      type == other.type &&
      (type != MapStyleType.raster ||
          (tileSize == other.tileSize && scheme == other.scheme));

  Map<String, Object> toJson() => {
    'id': id,
    'name': customName!,
    'url': url,
    'type': type.name,
    'tileSize': tileSize,
    'scheme': scheme.name,
  };

  factory MapStyle.fromJson(Map<String, dynamic> json) {
    final id = json['id'];
    final name = json['name'];
    final url = json['url'];
    final type = json['type'];
    final tileSize = json['tileSize'] ?? 256;
    final scheme = json['scheme'] ?? 'xyz';
    if (id is! String ||
        !id.startsWith('custom:') ||
        name is! String ||
        name.trim().isEmpty ||
        url is! String ||
        url.trim().isEmpty ||
        (type != 'style' && type != 'raster') ||
        tileSize is! int ||
        (tileSize != 256 && tileSize != 512) ||
        (scheme != 'xyz' && scheme != 'tms')) {
      throw const FormatException('Invalid custom basemap');
    }
    return MapStyle(
      id: id,
      customName: name,
      url: url,
      type: type == 'raster' ? MapStyleType.raster : MapStyleType.style,
      tileSize: tileSize,
      scheme: scheme == 'tms' ? MapTileScheme.tms : MapTileScheme.xyz,
      fogOpacityByStyle: const {
        MapFogStyle.dark: 0.50,
        MapFogStyle.light: 0.50,
      },
    );
  }

  /// Overlay opacity keyed by fog style ID, with values in the inclusive range
  /// 0–1.
  final Map<MapFogStyle, double> fogOpacityByStyle;

  // first one is the default.
  static const List<MapStyle> all = [
    MapStyle(
      id: 'openfreemap',
      url: 'https://tiles.openfreemap.org/styles/liberty',
      copyright: '[OpenFreeMap](https://openfreemap.org) [© OpenMapTiles](https://www.openmaptiles.org/) Data from [OpenStreetMap](https://www.openstreetmap.org/copyright)',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.50},
    ),
    MapStyle(
      id: 'maplibre',
      url: 'https://demotiles.maplibre.org/style.json',
      copyright: '[MapLibre](https://maplibre.org/)',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.50},
    ),
  ];

  static MapStyle findById(
    String? id, {
    List<MapStyle> customStyles = const [],
  }) {
    return all
        .followedBy(customStyles)
        .firstWhere((s) => s.id == id, orElse: () => all[0]);
  }
}
