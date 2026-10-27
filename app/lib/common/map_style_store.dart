import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/mmkv_util.dart';

/// Local basemap configuration. Source attribution is collected while the map
/// loads, rather than making requests from the settings editor.
class MapStyleStore extends ChangeNotifier {
  MapStyleStore({
    String? Function(String)? readString,
    bool Function(String, String?)? writeString,
  }) : _read = readString ?? MMKVUtil.getStringOpt,
       _write = writeString ?? MMKVUtil.putString;

  static final instance = MapStyleStore();

  final String? Function(String) _read;
  final bool Function(String, String?) _write;
  final Map<String, ({MapStyle style, List<String> entries})>
  _sourceAttributions = {};

  List<MapStyle> get customStyles {
    final stored = _read(MMKVKey.customMapStyles);
    if (stored == null) return [];
    try {
      final decoded = jsonDecode(stored);
      if (decoded is! List) return [];
      final styles = <String, MapStyle>{};
      for (final entry in decoded) {
        if (entry is! Map<String, dynamic>) continue;
        try {
          final style = MapStyle.fromJson(entry);
          styles[style.id] = style;
        } on FormatException {
          // A damaged entry must not hide the other saved maps.
        }
      }
      return styles.values.toList();
    } on FormatException {
      return [];
    }
  }

  MapStyle findById(String? id) =>
      MapStyle.findById(id, customStyles: customStyles);

  MapStyle get selected => findById(_read(MMKVKey.mapStyle));

  bool recordFallback(MapStyle style) {
    if (!style.isCustom || !selected.hasSameRenderer(style)) return false;
    select(MapStyle.all.first);
    return true;
  }

  void select(MapStyle style) {
    _put(MMKVKey.mapStyle, style.id);
    notifyListeners();
  }

  void save(MapStyle style, {bool selectAfterSaving = false}) {
    if (!style.isCustom || !style.id.startsWith('custom:')) {
      throw ArgumentError('Only custom basemaps can be saved');
    }
    final styles = customStyles;
    final index = styles.indexWhere((entry) => entry.id == style.id);
    final previous = index < 0 ? null : styles[index];
    if (index < 0) {
      styles.add(style);
    } else {
      styles[index] = style;
    }
    _put(
      MMKVKey.customMapStyles,
      jsonEncode(styles.map((s) => s.toJson()).toList()),
    );
    if (previous == null || !previous.hasSameRenderer(style)) {
      _sourceAttributions.remove(style.id);
    }
    if (selectAfterSaving) {
      select(style);
    } else {
      notifyListeners();
    }
  }

  void delete(String id) {
    final styles = customStyles.where((style) => style.id != id).toList();
    // Keep the selected ID valid even if only one write succeeds. A failed
    // selection write leaves the custom map intact for retry.
    if (_read(MMKVKey.mapStyle) == id) {
      _put(MMKVKey.mapStyle, MapStyle.all.first.id);
    }
    try {
      _put(
        MMKVKey.customMapStyles,
        jsonEncode(styles.map((s) => s.toJson()).toList()),
      );
      _sourceAttributions.remove(id);
    } finally {
      // A failed list write retains the configuration, but the persisted
      // default selection still needs to reach all mounted maps.
      notifyListeners();
    }
  }

  void recordSourceAttributions(MapStyle style, Iterable<String> entries) {
    if (!style.isCustom) return;
    if (!findById(style.id).hasSameRenderer(style)) return;
    _sourceAttributions[style.id] = (
      style: style,
      entries: entries
          .map((entry) => entry.trim())
          .where((e) => e.isNotEmpty)
          .toSet()
          .toList(),
    );
  }

  List<String> copyrightEntries(MapStyle style) {
    final loaded = _sourceAttributions[style.id];
    return {
      if (loaded?.style.hasSameRenderer(style) == true) ...loaded!.entries,
      if (style.copyright.trim().isNotEmpty) style.copyright.trim(),
    }.toList();
  }

  void _put(String key, String value) {
    if (!_write(key, value)) {
      throw StateError('Could not save basemap settings');
    }
  }
}
