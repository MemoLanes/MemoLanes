import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/src/rust/achievement/read_model/region.dart';

RegionEntity _entity({required String nameKey, String? isoA3Eh}) =>
    RegionEntity(
      kind: RegionKind.country,
      nameKey: RegionNameKey(value: nameKey),
      isoA3Eh: isoA3Eh,
      visitedAreaM2: BigInt.zero,
      totalAreaM2: BigInt.one,
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const loader = AppTranslationLoader();
  const enUs = Locale('en', 'US');

  setUpAll(() async {
    // easy_localization's ensureInitialized reads shared_preferences; stub the
    // platform channel so the test doesn't need the plugin as a direct dep.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
      const MethodChannel('plugins.flutter.io/shared_preferences'),
      (call) async => call.method == 'getAll' ? <String, Object>{} : null,
    );
    await EasyLocalization.ensureInitialized();
  });

  test('the loader merges region names nested like the UI translations',
      () async {
    // `flutter test` serves the real declared assets, so this exercises the
    // actual generated region_names.en-US.json merged with the UI translations.
    final map = await loader.load('assets/translations', enUs);

    // Nested levels, same shape as the UI files — easy_localization resolves
    // a dotted `name_key` by walking them.
    final country = map['country'] as Map<String, dynamic>;
    expect(country['CHN'], 'China');
    final continent = map['continent'] as Map<String, dynamic>;
    expect(continent['AS'], 'Asia');
    // UI translations survive the merge (disjoint top-level namespace).
    expect(map.containsKey('home'), isTrue);
  });

  testWidgets('displayName resolves via easy_localization, with fallbacks',
      (tester) async {
    // A real EasyLocalization + MaterialApp with the region-name loader — the
    // shipped path. The MaterialApp mounts the localization delegate, which is
    // what actually loads the merged translations into `Localization.instance`.
    final app = EasyLocalization(
      supportedLocales: const [enUs],
      path: 'assets/translations',
      assetLoader: loader,
      fallbackLocale: enUs,
      child: Builder(
        builder: (context) => MaterialApp(
          localizationsDelegates: context.localizationDelegates,
          supportedLocales: context.supportedLocales,
          locale: context.locale,
          home: const SizedBox.shrink(),
        ),
      ),
    );
    // The delegate loads assets via a real Future; runAsync lets it complete,
    // then pump/settle mounts the loaded translations into Localization.instance.
    await tester.runAsync(() async {
      await tester.pumpWidget(app);
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    // 1. localized name from the catalog.
    expect(
      _entity(nameKey: 'country.CHN', isoA3Eh: 'CHN').displayName('iso'),
      'China',
    );
    // 2. a key the catalog lacks → ISO code.
    expect(
      _entity(nameKey: 'country.NOPE', isoA3Eh: 'NOP').displayName('iso'),
      'NOP',
    );
    // 3. a miss with no ISO code → the raw key (never blank).
    expect(
      _entity(nameKey: 'continent.ZZ').displayName('iso'),
      'continent.ZZ',
    );
  });
}
