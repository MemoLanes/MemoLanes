import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/common/mmkv_util.dart';

MapStyle _custom({
  String id = 'one',
  String url = 'https://example.com/style.json?key=user-key',
  String name = 'My basemap',
  MapStyleType type = MapStyleType.style,
}) => MapStyle(
  ref: CustomMapStyleRef(id),
  customName: name,
  url: url,
  type: type,
  tileSize: 512,
  scheme: MapTileScheme.tms,
);

void main() {
  late Map<String, String> values;
  late MapStyleStore store;
  late String? failedKey;
  late List<String> writes;

  MapStyleStore openStore() => MapStyleStore(
    readString: (key) => values[key],
    writeString: (key, value) {
      writes.add(key);
      if (key == failedKey) return false;
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
    failedKey = null;
    writes = [];
    store = openStore();
  });

  test('selection and raster options survive restart without rewriting the URL', () {
    const url =
        'https://example.com/{z}/{x}/{y}.png?key=a+b&label="road"&path=\\tiles';
    final custom = _custom(url: url, type: MapStyleType.raster);
    store.save(custom, selectAfterSaving: true);
    expect(values[MMKVKey.mapStyle], 'custom:one');
    expect(jsonDecode(values[MMKVKey.customBasemaps]!), {
      'styles': [custom.toCustomJson()],
    });
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
      final second = _custom(id: 'two');
      store.save(first, selectAfterSaving: true);
      store.save(second);
      store.save(_custom(url: 'https://example.com/edited.json'));
      store.delete(const CustomMapStyleRef('two'));
      expect(store.customStyles.single.ref, first.ref);
      expect(store.selected.ref, first.ref);
      expect(store.selected.url, 'https://example.com/edited.json');
    },
  );

  test('selection and custom configurations are stored independently', () {
    final builtIn = MapStyle.all.last;
    final custom = _custom(id: builtIn.ref.id);
    values[MMKVKey.mapStyle] = builtIn.ref.id;

    store.save(custom);
    expect(writes, [MMKVKey.customBasemaps]);
    expect(openStore().selected, same(builtIn));
    final savedStyles = values[MMKVKey.customBasemaps];

    writes.clear();
    store.select(custom.ref);
    expect(writes, [MMKVKey.mapStyle]);
    expect(values[MMKVKey.mapStyle], 'custom:${custom.ref.id}');
    expect(openStore().selected.toCustomJson(), custom.toCustomJson());

    store.select(builtIn.ref);
    expect(openStore().selected, same(builtIn));
    expect(values[MMKVKey.customBasemaps], savedStyles);

    store.delete(CustomMapStyleRef(builtIn.ref.id));
    expect(values[MMKVKey.mapStyle], builtIn.ref.id);
    expect(openStore().selected, same(builtIn));
  });

  test(
    'damaged entries and unknown selections cannot hide built-in or valid maps',
    () {
      values[MMKVKey.mapStyle] = 'custom:missing';
      values[MMKVKey.customBasemaps] = jsonEncode({
        'styles': [
          null,
          {'id': 'openfreemap'},
          {..._custom().toCustomJson(), 'type': 'unsupported'},
          {..._custom().toCustomJson(), 'tileSize': 512.0},
          _custom().toCustomJson()..remove('tileSize'),
          _custom().toCustomJson()..remove('scheme'),
          _custom(id: 'valid').toCustomJson(),
        ],
      });
      expect(store.customStyles.map((s) => s.ref), [
        const CustomMapStyleRef('valid'),
      ]);
      expect(store.selected.ref, MapStyle.all.first.ref);

      values[MMKVKey.customBasemaps] = '{broken json';
      values[MMKVKey.mapStyle] = MapStyle.all.last.ref.id;
      expect(store.customStyles, isEmpty);
      expect(store.selected.ref, MapStyle.all.last.ref);
    },
  );

  test('failed changes preserve both the selection and the saved maps', () {
    final custom = _custom();
    final other = _custom(id: 'two');
    store.save(custom, selectAfterSaving: true);
    expect(writes, [MMKVKey.customBasemaps, MMKVKey.mapStyle]);

    failedKey = MMKVKey.customBasemaps;
    expect(() => store.save(other, selectAfterSaving: true), throwsStateError);
    expect(
      () => store.delete(const CustomMapStyleRef('one')),
      throwsStateError,
    );
    failedKey = MMKVKey.mapStyle;
    expect(() => store.select(MapStyle.all.last.ref), throwsStateError);
    expect(openStore().selected.toCustomJson(), custom.toCustomJson());
    expect(
      openStore().customStyles.single.toCustomJson(),
      custom.toCustomJson(),
    );

    writes.clear();
    failedKey = null;
    store.delete(const CustomMapStyleRef('one'));
    expect(writes, [MMKVKey.customBasemaps, MMKVKey.mapStyle]);
    expect(openStore().selected.ref, MapStyle.all.first.ref);
    expect(openStore().customStyles, isEmpty);
  });

  test('selection write failures leave saved configurations recoverable', () {
    final custom = _custom();
    store.select(MapStyle.all.last.ref);
    failedKey = MMKVKey.mapStyle;

    expect(() => store.save(custom, selectAfterSaving: true), throwsStateError);
    expect(openStore().selected.ref, MapStyle.all.last.ref);
    expect(
      openStore().customStyles.single.toCustomJson(),
      custom.toCustomJson(),
    );

    failedKey = null;
    store.save(custom, selectAfterSaving: true);
    expect(store.customStyles, hasLength(1));

    failedKey = MMKVKey.mapStyle;
    expect(
      () => store.delete(const CustomMapStyleRef('one')),
      throwsStateError,
    );
    expect(openStore().customStyles, isEmpty);
    expect(openStore().selected.ref, MapStyle.all.first.ref);

    failedKey = null;
    store.delete(const CustomMapStyleRef('one'));
    expect(values[MMKVKey.mapStyle], MapStyle.all.first.ref.id);
  });
}
