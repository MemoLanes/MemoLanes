import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/main.dart';
import 'package:provider/provider.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  tearDown(() => StyleConstants.setDarkMode(true));

  test(
    'missing and unknown preferences preserve the existing dark default',
    () {
      expect(AppThemePreference.fromId(null), AppThemePreference.dark);
      expect(AppThemePreference.fromId('unknown'), AppThemePreference.dark);
      for (final preference in AppThemePreference.values) {
        expect(AppThemePreference.fromId(preference.id), preference);
      }
      expect(StyleConstants.enableBackdropFilter, isFalse);
    },
  );

  test('preference persists, restores, and only notifies on change', () {
    String? stored;
    var writes = 0;
    final controller = AppThemeController(
      readPreference: () => stored,
      writePreference: (value) {
        stored = value;
        writes++;
      },
    );
    var notifications = 0;
    controller.addListener(() => notifications++);
    controller.setPreference(AppThemePreference.light);
    expect(StyleConstants.isDarkMode, isFalse);
    expect(controller.brightness, Brightness.light);
    controller.setPreference(AppThemePreference.light);
    expect(writes, 1);
    expect(notifications, 1);
    controller.dispose();
    final restored = AppThemeController(
      readPreference: () => stored,
      writePreference: (_) {},
    );
    addTearDown(restored.dispose);
    expect(restored.preference, AppThemePreference.light);
    expect(restored.brightness, Brightness.light);
  });

  test(
    'failed persistence does not change the active preference or palette',
    () {
      final controller = AppThemeController(
        readPreference: () => 'dark',
        writePreference: (_) => throw StateError('Storage failed'),
      );
      addTearDown(controller.dispose);
      expect(
        () => controller.setPreference(AppThemePreference.light),
        throwsStateError,
      );
      expect(controller.preference, AppThemePreference.dark);
      expect(StyleConstants.isDarkMode, isTrue);
    },
  );

  testWidgets('system brightness changes apply only in follow-system mode', (
    tester,
  ) async {
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
    final controller = AppThemeController(
      readPreference: () => 'system',
      writePreference: (_) {},
    );
    addTearDown(controller.dispose);
    expect(StyleConstants.isDarkMode, isFalse);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    await tester.pump();
    expect(StyleConstants.isDarkMode, isTrue);
    controller.setPreference(AppThemePreference.light);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    await tester.pump();
    expect(controller.brightness, Brightness.light);
    expect(StyleConstants.isDarkMode, isFalse);
  });

  testWidgets('Material theme changes without recreating the current page', (
    tester,
  ) async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    const locale = Locale('en', 'US');
    const loader = AppTranslationLoader();
    await EasyLocalization.ensureInitialized();
    final controller = AppThemeController(
      readPreference: () => 'dark',
      writePreference: (_) {},
    );
    addTearDown(controller.dispose);
    await tester.runAsync(() async {
      await loader.load('assets/translations', locale);
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale],
          path: 'assets/translations',
          assetLoader: loader,
          fallbackLocale: locale,
          child: ChangeNotifierProvider.value(
            value: controller,
            child: const MyApp(home: _StatefulThemeProbe()),
          ),
        ),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();
    final pageState = tester.state(find.byType(_StatefulThemeProbe));
    await tester.tap(find.text('0'));
    await tester.pump();
    for (final preference in [
      AppThemePreference.light,
      AppThemePreference.dark,
    ]) {
      controller.setPreference(preference);
      await tester.pumpAndSettle();
      final context = tester.element(find.byType(_StatefulThemeProbe));
      expect(Theme.of(context).brightness, controller.brightness);
      expect(
        Theme.of(context).scaffoldBackgroundColor,
        StyleConstants.canvasColor,
      );
      expect(tester.state(find.byType(_StatefulThemeProbe)), same(pageState));
      expect(find.text('1'), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _StatefulThemeProbe extends StatefulWidget {
  const _StatefulThemeProbe();

  @override
  State<_StatefulThemeProbe> createState() => _StatefulThemeProbeState();
}

class _StatefulThemeProbeState extends State<_StatefulThemeProbe> {
  int count = 0;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: TextButton(
      onPressed: () => setState(() => count++),
      child: Text('$count'),
    ),
  );
}
