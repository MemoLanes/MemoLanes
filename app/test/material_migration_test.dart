import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart' as legacy;
import 'package:flutter/services.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_date_picker_dialog.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/main.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:provider/provider.dart';

Future<void> _pumpApp(
  WidgetTester tester, {
  required AppThemeController controller,
  required Widget home,
  Locale locale = const Locale('en', 'US'),
}) async {
  await tester.runAsync(() async {
    await const AppTranslationLoader().load('assets/translations', locale);
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: const [Locale('en', 'US'), Locale('zh', 'CN')],
        path: 'assets/translations',
        assetLoader: const AppTranslationLoader(),
        fallbackLocale: const Locale('en', 'US'),
        startLocale: locale,
        saveLocale: false,
        child: ChangeNotifierProvider.value(
          value: controller,
          child: MyApp(home: home),
        ),
      ),
    );
    await tester.pump(const Duration(seconds: 1));
  });
  await tester.pumpAndSettle();
}

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

  setUp(() => AppHaptics.debugHapticsEnabledOverride = false);
  tearDown(() => AppHaptics.debugHapticsEnabledOverride = null);

  for (final (locale, cancel, confirm, month) in [
    (const Locale('en', 'US'), 'Cancel', 'OK', 'October'),
    (const Locale('zh', 'CN'), '取消', '确定', '十月'),
  ]) {
    testWidgets('calendar works with the app delegates in $locale', (
      tester,
    ) async {
      final controller = AppThemeController(
        readPreference: () => 'light',
        writePreference: (_) {},
      );
      addTearDown(controller.dispose);
      DateTime? selected;
      await _pumpApp(
        tester,
        controller: controller,
        locale: locale,
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () async {
                selected = await showAppDatePickerDialog(
                  context,
                  initialDate: DateTime(2026, 10, 17),
                  firstDate: DateTime(2026),
                  lastDate: DateTime(2026, 12, 31),
                );
              },
              child: const Text('Open calendar'),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Open calendar'));
      await tester.pumpAndSettle();
      expect(find.byType(CalendarDatePicker2), findsOneWidget);
      expect(find.text(cancel), findsOneWidget);
      expect(find.text(month), findsOneWidget);
      await tester.tap(find.text('18'));
      await tester.pumpAndSettle();
      await tester.tap(find.text(confirm));
      await tester.pumpAndSettle();
      expect(selected, DateTime(2026, 10, 18));
      expect(find.byType(CalendarDatePicker2), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  testWidgets('legacy Markdown follows the app light and dark themes', (
    tester,
  ) async {
    final controller = AppThemeController(
      readPreference: () => 'light',
      writePreference: (_) {},
    );
    addTearDown(controller.dispose);
    await _pumpApp(
      tester,
      controller: controller,
      home: const Scaffold(
        body: CommonDialog(
          title: 'Theme compatibility',
          content: 'Migration text',
          markdown: true,
        ),
      ),
    );

    for (final (preference, theme) in [
      (AppThemePreference.light, AppTheme.light),
      (AppThemePreference.dark, AppTheme.dark),
    ]) {
      controller.setPreference(preference);
      await tester.pumpAndSettle();
      final context = tester.element(find.byType(MarkdownBody));
      expect(legacy.Theme.of(context).brightness, theme.brightness);
      expect(
        legacy.Theme.of(context).colorScheme.primary,
        theme.colorScheme.primary,
      );
      final paragraph = tester.widget<RichText>(
        find
            .descendant(
              of: find.byType(MarkdownBody),
              matching: find.byType(RichText),
            )
            .first,
      );
      expect(paragraph.text.style?.color, theme.textTheme.bodyMedium?.color);
      expect(tester.takeException(), isNull);
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
