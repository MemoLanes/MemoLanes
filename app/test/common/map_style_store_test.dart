import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/common/mmkv_util.dart';

MapStyle _custom({
  String id = 'custom:one',
  String url = 'https://example.com/style.json?key=user-key',
  String name = 'My basemap',
  MapStyleType type = MapStyleType.style,
}) => MapStyle(
  id: id,
  customName: name,
  url: url,
  type: type,
  tileSize: 512,
  scheme: MapTileScheme.tms,
  fogOpacityByStyle: const {},
);

void main() {
  late Map<String, String> values;
  late MapStyleStore store;
  late Set<String> failedKeys;

  MapStyleStore openStore() => MapStyleStore(
    readString: (key) => values[key],
    writeString: (key, value) {
      if (failedKeys.contains(key)) return false;
      if (value == null) {
        values.remove(key);
      } else {
        values[key] = value;
      }
      return true;
    },
  );

  setUp(() {
    values = {};
    failedKeys = {};
    store = openStore();
  });

  test('selection and raster options survive restart without rewriting the URL', () {
    const url =
        'https://example.com/{z}/{x}/{y}.png?key=a+b&label="road"&path=\\tiles';
    final custom = _custom(url: url, type: MapStyleType.raster);
    store.save(custom, selectAfterSaving: true);
    final restored = openStore().selected;
    expect(restored.toJson(), custom.toJson());
    // These are the parameters JSON-encoded by the native WebView bridge.
    final injected = jsonDecode(jsonEncode(restored.mapLibreStyle));
    final source = injected['sources']['basemap'];
    expect(injected['version'], 8);
    expect(source, {
      'type': 'raster',
      'tiles': [url],
      'tileSize': 512,
      'scheme': 'tms',
    });
    expect(injected['layers'].single['source'], 'basemap');
  });

  test('editing and deleting an inactive map preserve the selection', () {
    final first = _custom();
    final second = _custom(id: 'custom:two');
    store.save(first, selectAfterSaving: true);
    store.save(second);
    store.save(_custom(url: 'https://example.com/edited.json'));
    store.delete(second.id);
    expect(store.customStyles.single.id, first.id);
    expect(store.selected.id, first.id);
    expect(store.selected.url, 'https://example.com/edited.json');
  });

  test(
    'damaged entries and unknown selections cannot hide built-in or valid maps',
    () {
      values[MMKVKey.customMapStyles] = jsonEncode([
        null,
        {'id': 'openfreemap'},
        {..._custom().toJson(), 'type': 'unsupported'},
        {..._custom().toJson(), 'tileSize': 512.0},
        _custom(id: 'custom:valid').toJson(),
      ]);
      values[MMKVKey.mapStyle] = 'custom:missing';
      expect(store.customStyles.map((s) => s.id), ['custom:valid']);
      expect(store.selected.id, MapStyle.all.first.id);

      values[MMKVKey.customMapStyles] = '{broken json';
      expect(store.customStyles, isEmpty);
      expect(store.selected.id, MapStyle.all.first.id);
    },
  );

  test(
    'loaded source attribution follows renderer changes and rejects stale maps',
    () {
      final first = _custom();
      store.save(first);
      store.recordSourceAttributions(first, [
        ' Source credit ',
        '',
        'Source credit',
      ]);
      expect(store.copyrightEntries(first), ['Source credit']);

      final renamed = _custom(name: 'Renamed map');
      store.save(renamed);
      expect(store.copyrightEntries(renamed), ['Source credit']);

      final edited = _custom(url: 'https://other.example.com/style.json');
      store.save(edited);
      store.recordSourceAttributions(first, ['Late old source']);
      expect(store.copyrightEntries(edited), isEmpty);
      store.recordSourceAttributions(edited, ['New source']);
      expect(store.copyrightEntries(edited), ['New source']);

      store.delete(edited.id);
      store.recordSourceAttributions(edited, ['Late deleted source']);
      store.save(edited);
      expect(store.copyrightEntries(edited), isEmpty);
    },
  );

  test('failed saves and selections preserve the current map', () {
    final custom = _custom();
    store.save(custom, selectAfterSaving: true);
    failedKeys.addAll([MMKVKey.customMapStyles, MMKVKey.mapStyle]);
    expect(
      () => store.save(_custom(url: 'https://example.com/new.json')),
      throwsStateError,
    );
    expect(() => store.select(MapStyle.all.last), throwsStateError);
    expect(store.selected.toJson(), custom.toJson());
  });

  for (final key in [MMKVKey.mapStyle, MMKVKey.customMapStyles]) {
    test('failed delete write to $key retains the map for retry', () {
      final custom = _custom();
      store.save(custom, selectAfterSaving: true);
      store.recordSourceAttributions(custom, ['Loaded credit']);
      var displayed = store.selected;
      store.addListener(() => displayed = store.selected);
      failedKeys.add(key);

      expect(() => store.delete(custom.id), throwsStateError);
      final expectedId = key == MMKVKey.mapStyle
          ? custom.id
          : MapStyle.all.first.id;
      expect(openStore().selected.id, expectedId);
      expect(displayed.id, expectedId);
      expect(store.customStyles.single.id, custom.id);
      expect(store.copyrightEntries(custom), ['Loaded credit']);

      failedKeys.clear();
      store.delete(custom.id);
      expect(openStore().selected.id, MapStyle.all.first.id);
      expect(store.customStyles, isEmpty);
      expect(store.copyrightEntries(custom), isEmpty);
    });
  }

  test('fallback persists the default and notifies all mounted maps', () {
    final custom = _custom();
    store.save(custom, selectAfterSaving: true);
    final firstMap = <String>[];
    final secondMap = <String>[];
    store.addListener(() => firstMap.add(store.selected.id));
    store.addListener(() => secondMap.add(store.selected.id));
    expect(store.recordFallback(custom), isTrue);
    expect(openStore().selected.id, MapStyle.all.first.id);
    expect(firstMap, [MapStyle.all.first.id]);
    expect(secondMap, firstMap);
    expect(store.customStyles.single.id, custom.id);

    store.select(custom);
    store.save(_custom(url: 'https://example.com/edited.json'));
    expect(store.recordFallback(custom), isFalse);
    expect(store.selected.id, custom.id);
  });
}
