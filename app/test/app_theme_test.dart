import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/main.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:provider/provider.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
  });

  test('saved preference is restored by a new controller', () {
    String? stored;
    final controller = AppThemeController(
      readPreference: () => stored,
      writePreference: (value) => stored = value,
    );
    controller.setPreference(AppThemePreference.light);
    controller.dispose();
    final restored = AppThemeController(
      readPreference: () => stored,
      writePreference: (_) {},
    );
    addTearDown(restored.dispose);
    expect(restored.preference, AppThemePreference.light);
    expect(restored.themeMode, ThemeMode.light);
  });

  test('preferences map to Flutter theme modes', () {
    final controller = AppThemeController(
      readPreference: () => null,
      writePreference: (_) {},
    );
    addTearDown(controller.dispose);
    expect(controller.preference, AppThemePreference.system);
    expect(controller.themeMode, ThemeMode.system);
    controller.setPreference(AppThemePreference.light);
    expect(controller.themeMode, ThemeMode.light);
    controller.setPreference(AppThemePreference.dark);
    expect(controller.themeMode, ThemeMode.dark);
  });

  test('map status icons stay light without changing navigation bar style', () {
    for (final theme in [AppTheme.light, AppTheme.dark]) {
      final appStyle = theme.appBarTheme.systemOverlayStyle!;
      final mapStyle = AppTheme.mapSystemOverlayStyle(theme);

      expect(mapStyle.statusBarIconBrightness, Brightness.light);
      expect(mapStyle.statusBarBrightness, Brightness.dark);
      expect(mapStyle.statusBarColor, appStyle.statusBarColor);
      expect(
        mapStyle.systemNavigationBarColor,
        appStyle.systemNavigationBarColor,
      );
      expect(
        mapStyle.systemNavigationBarIconBrightness,
        appStyle.systemNavigationBarIconBrightness,
      );
      expect(
        mapStyle.systemNavigationBarDividerColor,
        appStyle.systemNavigationBarDividerColor,
      );
    }
  });

  testWidgets(
    'system and manual modes update const children without losing state',
    (tester) async {
      tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
      addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
      const locale = Locale('en', 'US');
      const loader = AppTranslationLoader();
      final controller = AppThemeController(
        readPreference: () => 'system',
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

      expect(_probeBrightness(tester), Brightness.light);
      tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
      await tester.pumpAndSettle();
      expect(_probeBrightness(tester), Brightness.dark);

      controller.setPreference(AppThemePreference.light);
      await tester.pumpAndSettle();
      expect(_probeBrightness(tester), Brightness.light);

      tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
      await tester.pumpAndSettle();
      expect(_probeBrightness(tester), Brightness.light);

      controller.setPreference(AppThemePreference.dark);
      await tester.pumpAndSettle();
      expect(_probeBrightness(tester), Brightness.dark);
      expect(tester.state(find.byType(_StatefulThemeProbe)), same(pageState));
      expect(find.text('1'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Brightness _probeBrightness(WidgetTester tester) {
  final value = tester.widget<Text>(find.byKey(_ConstThemeProbe.probeKey)).data;
  return Brightness.values.byName(value!);
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
    body: Column(
      children: [
        TextButton(
          onPressed: () => setState(() => count++),
          child: Text('$count'),
        ),
        const _ConstThemeProbe(),
      ],
    ),
  );
}

class _ConstThemeProbe extends StatelessWidget {
  const _ConstThemeProbe();

  static const probeKey = ValueKey('const-theme-probe');

  @override
  Widget build(BuildContext context) {
    return Text(Theme.of(context).brightness.name, key: probeKey);
  }
}
