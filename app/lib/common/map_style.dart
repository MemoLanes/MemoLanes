import 'package:memolanes/common/map_fog_style.dart';

enum MapStyleType { style, raster }

enum MapTileScheme { xyz, tms }

sealed class MapStyleRef {
  const MapStyleRef(this.id);

  static const _customPrefix = 'custom:';
  final String id;

  factory MapStyleRef.fromStorage(String value) =>
      value.startsWith(_customPrefix)
      ? CustomMapStyleRef(value.substring(_customPrefix.length))
      : BuiltInMapStyleRef(value);

  String toStorage() => switch (this) {
    BuiltInMapStyleRef() => id,
    CustomMapStyleRef() => '$_customPrefix$id',
  };

  @override
  bool operator ==(Object other) =>
      other is MapStyleRef &&
      runtimeType == other.runtimeType &&
      id == other.id;

  @override
  int get hashCode => Object.hash(runtimeType, id);
}

final class BuiltInMapStyleRef extends MapStyleRef {
  const BuiltInMapStyleRef(super.id);
}

final class CustomMapStyleRef extends MapStyleRef {
  const CustomMapStyleRef(super.id);
}

/// A basemap's identity, source configuration, and fog opacity.
class MapStyle {
  const MapStyle({
    required this.ref,
    required this.url,
    this.fogOpacityByStyle = const {},
    this.customName,
    this.type = MapStyleType.style,
    this.tileSize = 256,
    this.scheme = MapTileScheme.xyz,
  }) : assert((ref is CustomMapStyleRef) == (customName != null));

  final MapStyleRef ref;
  final String url;

  final String? customName;
  final MapStyleType type;
  final int tileSize;
  final MapTileScheme scheme;

  bool get isCustom => ref is CustomMapStyleRef;

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
    if (!isCustom || name == null) {
      throw StateError('Only custom basemaps can be serialized');
    }
    return {
      'id': ref.id,
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
      ref: CustomMapStyleRef(id),
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
      ref: BuiltInMapStyleRef('openfreemap'),
      url: 'https://tiles.openfreemap.org/styles/liberty',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.50},
    ),
    MapStyle(
      ref: BuiltInMapStyleRef('maplibre'),
      url: 'https://demotiles.maplibre.org/style.json',
      fogOpacityByStyle: {MapFogStyle.dark: 0.50, MapFogStyle.light: 0.50},
    ),
  ];
}
