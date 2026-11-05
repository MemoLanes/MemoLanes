import 'dart:convert';

import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/mmkv_util.dart';

class MapStyleStore {
  MapStyleStore({
    String? Function(String)? readString,
    bool Function(String, String?)? writeString,
  }) : _read = readString ?? MMKVUtil.getStringOpt,
       _write = writeString ?? MMKVUtil.putString;

  static final instance = MapStyleStore();

  final String? Function(String) _read;
  final bool Function(String, String?) _write;

  ({String? selectedId, List<MapStyle> styles}) _load() {
    final stored = _read(MMKVKey.basemaps);
    if (stored == null) {
      return (selectedId: _read(MMKVKey.mapStyle), styles: []);
    }
    try {
      final decoded = jsonDecode(stored);
      if (decoded is! Map<String, dynamic> ||
          decoded['selectedId'] is! String) {
        throw const FormatException('Invalid basemap settings');
      }
      return (
        selectedId: decoded['selectedId'] as String,
        styles: _decodeStyles(decoded['styles']),
      );
    } on FormatException catch (error, stackTrace) {
      log.error(
        'Reading basemap settings failed: ${error.message}',
        stackTrace,
      );
      return (selectedId: null, styles: []);
    }
  }

  List<MapStyle> _decodeStyles(Object? decoded) {
    if (decoded is! List) throw const FormatException('Expected basemap list');
    final styles = <String, MapStyle>{};
    for (final entry in decoded) {
      try {
        if (entry is! Map<String, dynamic>) {
          throw const FormatException('Expected basemap object');
        }
        final style = MapStyle.fromCustomJson(entry);
        styles[style.id] = style;
      } on FormatException catch (error, stackTrace) {
        log.error('Reading saved basemap failed: ${error.message}', stackTrace);
      }
    }
    return styles.values.toList();
  }

  List<MapStyle> get customStyles => _load().styles;

  MapStyle get selected {
    final current = _load();
    return MapStyle.findById(current.selectedId, customStyles: current.styles);
  }

  void select(MapStyle style) => _save(_load().styles, style.id);

  void save(MapStyle style, {bool selectAfterSaving = false}) {
    if (!style.isCustom || !style.id.startsWith('custom:')) {
      throw ArgumentError('Only custom basemaps can be saved');
    }
    final current = _load();
    final styles = current.styles;
    final index = styles.indexWhere((entry) => entry.id == style.id);
    if (index < 0) {
      styles.add(style);
    } else {
      styles[index] = style;
    }
    _save(styles, selectAfterSaving ? style.id : current.selectedId);
  }

  void delete(String id) {
    final current = _load();
    final styles = current.styles.where((style) => style.id != id).toList();
    _save(styles, current.selectedId == id ? null : current.selectedId);
  }

  void _save(List<MapStyle> styles, String? selectedId) {
    final value = jsonEncode({
      'selectedId': selectedId ?? MapStyle.all.first.id,
      'styles': styles.map((style) => style.toCustomJson()).toList(),
    });
    if (!_write(MMKVKey.basemaps, value)) {
      throw StateError('Could not save basemap settings');
    }
  }
}
