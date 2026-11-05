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
);

void main() {
  late Map<String, String> values;
  late MapStyleStore store;
  late bool failWrite;
  late List<String> writes;

  MapStyleStore openStore() => MapStyleStore(
    readString: (key) => values[key],
    writeString: (key, value) {
      writes.add(key);
      if (failWrite) return false;
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
    failWrite = false;
    writes = [];
    store = openStore();
  });

  test('selection and raster options survive restart without rewriting the URL', () {
    const url =
        'https://example.com/{z}/{x}/{y}.png?key=a+b&label="road"&path=\\tiles';
    final custom = _custom(url: url, type: MapStyleType.raster);
    store.save(custom, selectAfterSaving: true);
    final restored = openStore().selected;
    expect(restored.toCustomJson(), custom.toCustomJson());
    final style = restored.mapLibreStyle as Map<String, dynamic>;
    final source = style['sources']['basemap'];
    expect(style['version'], 8);
    expect(source, {
      'type': 'raster',
      'tiles': [url],
      'tileSize': 512,
      'scheme': 'tms',
    });
    expect(style['layers'].single['source'], 'basemap');
  });

  test(
    'editing the selected map and deleting another preserve the selection',
    () {
      final first = _custom();
      final second = _custom(id: 'custom:two');
      store.save(first, selectAfterSaving: true);
      store.save(second);
      store.save(_custom(url: 'https://example.com/edited.json'));
      store.delete(second.id);
      expect(store.customStyles.single.id, first.id);
      expect(store.selected.id, first.id);
      expect(store.selected.url, 'https://example.com/edited.json');
    },
  );

  test('preserves the built-in selection from main on the first write', () {
    final custom = _custom();
    values[MMKVKey.mapStyle] = MapStyle.all.last.id;
    expect(store.selected.id, MapStyle.all.last.id);
    expect(store.customStyles, isEmpty);
    expect(writes, isEmpty);

    store.save(custom);
    values[MMKVKey.mapStyle] = MapStyle.all.first.id;
    final reopened = openStore();
    expect(writes, [MMKVKey.basemaps]);
    expect(reopened.selected.id, MapStyle.all.last.id);
    expect(reopened.customStyles.single.toCustomJson(), custom.toCustomJson());
  });

  test(
    'damaged entries and unknown selections cannot hide built-in or valid maps',
    () {
      values[MMKVKey.basemaps] = jsonEncode({
        'selectedId': 'custom:missing',
        'styles': [
          null,
          {'id': 'openfreemap'},
          {..._custom().toCustomJson(), 'type': 'unsupported'},
          {..._custom().toCustomJson(), 'tileSize': 512.0},
          _custom().toCustomJson()..remove('tileSize'),
          _custom().toCustomJson()..remove('scheme'),
          _custom(id: 'custom:valid').toCustomJson(),
        ],
      });
      expect(store.customStyles.map((s) => s.id), ['custom:valid']);
      expect(store.selected.id, MapStyle.all.first.id);

      values[MMKVKey.basemaps] = '{broken json';
      expect(store.customStyles, isEmpty);
      expect(store.selected.id, MapStyle.all.first.id);
    },
  );

  test('failed changes preserve both the selection and the saved maps', () {
    final custom = _custom();
    final other = _custom(id: 'custom:two');
    store.save(custom, selectAfterSaving: true);
    expect(writes, [MMKVKey.basemaps]);

    failWrite = true;
    expect(() => store.save(other, selectAfterSaving: true), throwsStateError);
    expect(() => store.select(MapStyle.all.last), throwsStateError);
    expect(() => store.delete(custom.id), throwsStateError);
    expect(openStore().selected.toCustomJson(), custom.toCustomJson());
    expect(
      openStore().customStyles.single.toCustomJson(),
      custom.toCustomJson(),
    );

    writes.clear();
    failWrite = false;
    store.delete(custom.id);
    expect(writes, [MMKVKey.basemaps]);
    expect(openStore().selected.id, MapStyle.all.first.id);
    expect(openStore().customStyles, isEmpty);
  });
}
