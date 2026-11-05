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

  List<MapStyle> get customStyles {
    final stored = _read(MMKVKey.customBasemaps);
    if (stored == null) return [];
    try {
      final decoded = jsonDecode(stored);
      if (decoded is! Map<String, dynamic>) {
        throw const FormatException('Invalid custom basemap settings');
      }
      return _decodeStyles(decoded['styles']);
    } on FormatException catch (error, stackTrace) {
      log.error(
        'Reading custom basemap settings failed: ${error.message}',
        stackTrace,
      );
      return [];
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

  MapStyle get selected {
    final selectionId = _read(MMKVKey.mapStyle);
    final styles = selectionId?.startsWith('custom:') == true
        ? customStyles
        : MapStyle.all;
    return styles.firstWhere(
      (style) => style.selectionId == selectionId,
      orElse: () => MapStyle.all.first,
    );
  }

  void select(MapStyle style) {
    if (!_write(MMKVKey.mapStyle, style.selectionId)) {
      throw StateError('Could not save basemap selection');
    }
  }

  void save(MapStyle style, {bool selectAfterSaving = false}) {
    if (!style.isCustom) {
      throw ArgumentError('Only custom basemaps can be saved');
    }
    final styles = customStyles;
    final index = styles.indexWhere((entry) => entry.id == style.id);
    if (index < 0) {
      styles.add(style);
    } else {
      styles[index] = style;
    }
    _saveStyles(styles);
    if (selectAfterSaving) select(style);
  }

  void delete(String id) {
    _saveStyles(customStyles.where((style) => style.id != id).toList());
    if (_read(MMKVKey.mapStyle) == 'custom:$id') {
      select(MapStyle.all.first);
    }
  }

  void _saveStyles(List<MapStyle> styles) {
    final value = jsonEncode({
      'styles': styles.map((style) => style.toCustomJson()).toList(),
    });
    if (!_write(MMKVKey.customBasemaps, value)) {
      throw StateError('Could not save custom basemaps');
    }
  }
}
