import 'dart:ui' show Locale;

import 'package:flutter_test/flutter_test.dart';
import 'package:logging/logging.dart';
import 'package:memolanes/common/region_preference.dart';

class _Preferences {
  Worldview? saved;
  int setupCompletedVersion = 0;
  List<Locale> locales = const [Locale('zh', 'CN')];
  final activations = <Worldview>[];
  final writes = <Worldview>[];
  bool failActivation = false;
  bool failPersistence = false;

  WorldviewManager createManager() => WorldviewManager.forTesting(
    readSavedWorldview: () => saved,
    hasConfirmedPreference: () => setupCompletedVersion >= 1,
    readDeviceLocales: () => locales,
    activateGeoData: (worldview) async {
      activations.add(worldview);
      if (failActivation) throw StateError('activation failed');
    },
    persistWorldview: (worldview) {
      if (failPersistence) throw StateError('persistence failed');
      writes.add(worldview);
      saved = worldview;
    },
  );
}

void main() {
  group('worldview recommendations', () {
    test('maps explicit regions independently of language and script', () {
      for (final (locale, expected) in const [
        (Locale('zh', 'CN'), Worldview.chn),
        (Locale('en', 'HK'), Worldview.iso),
        (Locale('pt', 'MO'), Worldview.iso),
        (
          Locale.fromSubtags(
            languageCode: 'zh',
            scriptCode: 'Hans',
            countryCode: 'HK',
          ),
          Worldview.iso,
        ),
        (
          Locale.fromSubtags(
            languageCode: 'zh',
            scriptCode: 'Hant',
            countryCode: 'HK',
          ),
          Worldview.iso,
        ),
        (Locale('zh', 'US'), Worldview.usa),
        (Locale('zh', 'TW'), Worldview.iso),
        (Locale('zh', 'SG'), Worldview.iso),
        (Locale('en', 'GB'), Worldview.iso),
      ]) {
        expect(
          defaultWorldviewFromLocales([locale]),
          expected,
          reason: '$locale',
        );
      }
    });

    test('missing region falls back to ISO, not language or script', () {
      expect(defaultWorldviewFromLocales([]), Worldview.iso);
      expect(
        defaultWorldviewFromLocales(const [
          Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans'),
          Locale('en'),
        ]),
        Worldview.iso,
      );
    });

    test('uses the first explicit region, even when it maps to ISO', () {
      expect(
        defaultWorldviewFromLocales(const [Locale('zh'), Locale('en', 'HK')]),
        Worldview.iso,
      );
      expect(
        defaultWorldviewFromLocales(const [
          Locale('en', 'GB'),
          Locale('zh', 'CN'),
        ]),
        Worldview.iso,
      );
    });
  });

  group('worldview confirmation', () {
    late _Preferences preferences;
    late WorldviewManager manager;

    setUp(() {
      preferences = _Preferences();
      manager = preferences.createManager();
    });

    test(
      'recommendation is saved only on acceptance and survives restart',
      () async {
        await manager.initialize();
        expect(manager.currentWorldview, Worldview.chn);
        expect(preferences.writes, isEmpty);

        await manager.update(Worldview.chn);
        expect(preferences.saved, Worldview.chn);
        expect(preferences.activations, [Worldview.chn]);

        // First-launch setup records completion after accepting the choice.
        preferences.setupCompletedVersion = 1;
        preferences.locales = const [Locale('en', 'US')];
        final nextLaunch = preferences.createManager();
        await nextLaunch.initialize();
        expect(nextLaunch.currentWorldview, Worldview.chn);
      },
    );

    test('preserves a previously confirmed ISO choice', () async {
      preferences.saved = Worldview.iso;
      preferences.setupCompletedVersion = 1;
      await manager.initialize();
      expect(manager.currentWorldview, Worldview.iso);
      expect(preferences.writes, isEmpty);
    });

    test(
      'ignores legacy unconfirmed values and recomputes on restart',
      () async {
        preferences.saved = Worldview.iso;
        await manager.initialize();
        expect(manager.currentWorldview, Worldview.chn);
        expect(preferences.writes, isEmpty);

        preferences.locales = const [Locale('en', 'US')];
        final nextLaunch = preferences.createManager();
        await nextLaunch.initialize();
        expect(nextLaunch.currentWorldview, Worldview.usa);
        expect(preferences.writes, isEmpty);
      },
    );

    test('activation failure preserves the old confirmed choice', () async {
      await manager.initialize();
      await manager.update(Worldview.chn);
      preferences.failActivation = true;
      await expectLater(manager.update(Worldview.usa), throwsStateError);
      expect(manager.currentWorldview, Worldview.chn);
      expect(preferences.saved, Worldview.chn);
      expect(preferences.writes, [Worldview.chn]);
    });

    test(
      'save failure logs an error but keeps the active choice and allows retry',
      () async {
        final records = <LogRecord>[];
        final subscription = Logger.root.onRecord.listen(records.add);
        addTearDown(subscription.cancel);
        await manager.initialize();
        preferences.failPersistence = true;
        await manager.update(Worldview.usa);
        expect(manager.currentWorldview, Worldview.usa);
        expect(preferences.saved, isNull);
        expect(records.any((record) => record.level == Level.SEVERE), isTrue);

        preferences.failPersistence = false;
        await manager.update(Worldview.usa);
        expect(preferences.saved, Worldview.usa);
        expect(preferences.activations, [Worldview.chn, Worldview.usa]);
      },
    );

    test(
      'returning to the old choice after a failed save persists again',
      () async {
        await manager.initialize();
        await manager.update(Worldview.chn);
        preferences.failPersistence = true;
        await manager.update(Worldview.usa);
        preferences.failPersistence = false;
        await manager.update(Worldview.chn);
        expect(manager.currentWorldview, Worldview.chn);
        expect(preferences.writes, [Worldview.chn, Worldview.chn]);
      },
    );
  });
}
