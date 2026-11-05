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
    final styles = <MapStyleRef, MapStyle>{};
    for (final entry in decoded) {
      try {
        if (entry is! Map<String, dynamic>) {
          throw const FormatException('Expected basemap object');
        }
        final style = MapStyle.fromCustomJson(entry);
        styles[style.ref] = style;
      } on FormatException catch (error, stackTrace) {
        log.error('Reading saved basemap failed: ${error.message}', stackTrace);
      }
    }
    return styles.values.toList();
  }

  MapStyleRef get _selectedRef {
    final value = _read(MMKVKey.mapStyle);
    return value == null
        ? MapStyle.all.first.ref
        : MapStyleRef.fromStorage(value);
  }

  MapStyle get selected {
    final ref = _selectedRef;
    final styles = switch (ref) {
      BuiltInMapStyleRef() => MapStyle.all,
      CustomMapStyleRef() => customStyles,
    };
    return styles.firstWhere(
      (style) => style.ref == ref,
      orElse: () => MapStyle.all.first,
    );
  }

  void select(MapStyleRef ref) {
    if (!_write(MMKVKey.mapStyle, ref.toStorage())) {
      throw StateError('Could not save basemap selection');
    }
  }

  void save(MapStyle style, {bool selectAfterSaving = false}) {
    if (!style.isCustom) {
      throw ArgumentError('Only custom basemaps can be saved');
    }
    final styles = customStyles;
    final index = styles.indexWhere((entry) => entry.ref == style.ref);
    if (index < 0) {
      styles.add(style);
    } else {
      styles[index] = style;
    }
    _saveStyles(styles);
    if (selectAfterSaving) select(style.ref);
  }

  void delete(CustomMapStyleRef ref) {
    _saveStyles(customStyles.where((style) => style.ref != ref).toList());
    if (_selectedRef == ref) {
      select(MapStyle.all.first.ref);
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
