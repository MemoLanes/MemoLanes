import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/region_preference.dart';

class _Preferences {
  Worldview? saved;
  String deviceRegion = 'CN';
  final activations = <Worldview>[];
  bool failActivation = false;
  bool failPersistence = false;

  WorldviewManager createManager() => WorldviewManager.forTesting(
    readSavedWorldview: () => saved,
    readDeviceRegion: () async => deviceRegion,
    activateGeoData: (worldview) async {
      activations.add(worldview);
      if (failActivation) throw StateError('activation failed');
    },
    persistWorldview: (worldview) {
      if (failPersistence) throw StateError('persistence failed');
      saved = worldview;
    },
  );
}

void main() {
  group('worldview preference persistence', () {
    late _Preferences preferences;
    late WorldviewManager manager;

    setUp(() {
      preferences = _Preferences();
      manager = preferences.createManager();
    });

    test(
      'accepting the active recommendation saves it and survives restart',
      () async {
        preferences.deviceRegion = 'US';
        await manager.initialize();
        expect(manager.currentWorldview, Worldview.usa);
        expect(preferences.saved, isNull);

        await manager.update(Worldview.usa);
        expect(preferences.saved, Worldview.usa);
        expect(preferences.activations, [Worldview.usa]);

        preferences.deviceRegion = 'CN';
        final nextLaunch = preferences.createManager();
        await nextLaunch.initialize();
        expect(nextLaunch.currentWorldview, Worldview.usa);
      },
    );

    test('activation failure preserves active and saved choices', () async {
      preferences.saved = Worldview.chn;
      await manager.initialize();
      preferences.failActivation = true;
      await expectLater(manager.update(Worldview.usa), throwsStateError);
      expect(manager.currentWorldview, Worldview.chn);
      expect(preferences.saved, Worldview.chn);
    });

    test(
      'save failure keeps the active choice and permits same-choice retry',
      () async {
        preferences.saved = Worldview.chn;
        await manager.initialize();
        preferences.failPersistence = true;
        await manager.update(Worldview.usa);
        expect(manager.currentWorldview, Worldview.usa);
        expect(preferences.saved, Worldview.chn);

        preferences.failPersistence = false;
        await manager.update(Worldview.usa);
        expect(preferences.saved, Worldview.usa);
        expect(preferences.activations, [Worldview.chn, Worldview.usa]);
      },
    );
  });
}
