import 'package:memolanes/common/map_fog_style.dart';

enum MapStyleType { style, raster }

enum MapTileScheme { xyz, tms }

/// Map base style: id, url, and per-fog-style opacity.
/// [selectionId] distinguishes built-in and custom styles in saved preferences.
class MapStyle {
  const MapStyle({
    required this.id,
    required this.url,
    this.fogOpacityByStyle = const {},
    this.customName,
    this.type = MapStyleType.style,
    this.tileSize = 256,
    this.scheme = MapTileScheme.xyz,
  });

  final String id;
  final String url;

  final String? customName;
  final MapStyleType type;
  final int tileSize;
  final MapTileScheme scheme;

  bool get isCustom => customName != null;

  String get selectionId => isCustom ? 'custom:$id' : id;

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

  Map<String, Object> toCustomJson() {
    final name = customName;
    if (name == null) {
      throw StateError('Only custom basemaps can be serialized');
    }
    return {
      'id': id,
      'name': name,
      'url': url,
      'type': type.name,
      'tileSize': tileSize,
      'scheme': scheme.name,
    };
  }

  factory MapStyle.fromCustomJson(Map<String, dynamic> json) {
    final id = json['id'];
    final name = json['name'];
    final url = json['url'];
    final type = json['type'];
    final tileSize = json['tileSize'];
    final scheme = json['scheme'];
    if (id is! String ||
        id.trim().isEmpty ||
        id.startsWith('custom:') ||
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
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.50},
    ),
    MapStyle(
      id: 'maplibre',
      url: 'https://demotiles.maplibre.org/style.json',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.50},
    ),
  ];
}
