import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/settings/settings_body.dart';
import 'package:memolanes/body/settings/appearance_picker.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:provider/provider.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    PackageInfo.setMockInitialValues(
      appName: 'MemoLanes',
      packageName: 'com.memolanes',
      version: '1.2.3',
      buildNumber: '45',
      buildSignature: '',
    );
    RustLib.initMock(api: _SettingsApi());
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
  });
  tearDownAll(RustLib.dispose);

  testWidgets('settings retains six categories and scrollable safe insets', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [Locale('en', 'US')],
          path: 'assets/translations',
          saveLocale: false,
          child: Builder(
            builder: (context) => MaterialApp(
              theme: AppTheme.light,
              locale: context.locale,
              supportedLocales: context.supportedLocales,
              localizationsDelegates: context.localizationDelegates,
              home: const Scaffold(
                body: SettingsBody(topSafeArea: 32, bottomSafeArea: 90),
              ),
            ),
          ),
        ),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    final layout = tester.widget<SettingsPageLayout>(
      find.byType(SettingsPageLayout),
    );
    final cards = tester
        .widgetList<OptionCard>(find.byType(OptionCard))
        .toList();
    expect(cards, hasLength(3));
    expect(
      cards.every((card) => !card.useSafeArea && !card.separators),
      isTrue,
    );
    expect(layout.topPadding, 56);
    expect(layout.bottomPadding, 106);
    expect(layout.framePadding, const EdgeInsets.symmetric(horizontal: 8));
    expect(tester.getTopLeft(find.byType(OptionCard).first).dx, 8);
    expect(tester.getTopLeft(find.text('Settings')).dy, 56);
    final tiles = tester.widgetList<LabelTile>(find.byType(LabelTile)).toList();
    expect(tiles.map((tile) => tile.label), [
      'Journey & Recording',
      'Map Settings',
      'Appearance',
      'Data',
      'Advanced',
      'About MemoLanes',
    ]);
    expect(tiles.every((tile) => tile.onTap != null), isTrue);
    await tester.drag(
      find.byType(SingleChildScrollView),
      const Offset(0, -1000),
    );
    await tester.pumpAndSettle();
    expect(find.text('About MemoLanes').hitTestable(), findsOneWidget);
    final version = find.text('1.2.3 (45) [abc1234]');
    expect(version.hitTestable(), findsOneWidget);
    expect(
      tester.getTopLeft(version).dy,
      greaterThan(tester.getBottomLeft(find.byType(LabelTile).last).dy),
    );
    expect(
      tester.getBottomLeft(find.byType(LabelTile).last).dy,
      lessThanOrEqualTo(494),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('appearance picker offers system, light, and dark modes', (
    tester,
  ) async {
    final saved = <String>[];
    final controller = AppThemeController(
      readPreference: () => null,
      writePreference: saved.add,
    );
    addTearDown(controller.dispose);

    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [Locale('en', 'US')],
          path: 'assets/translations',
          saveLocale: false,
          child: ChangeNotifierProvider.value(
            value: controller,
            child: Builder(
              builder: (context) => MaterialApp(
                theme: AppTheme.light,
                locale: context.locale,
                supportedLocales: context.supportedLocales,
                localizationsDelegates: context.localizationDelegates,
                home: Scaffold(
                  body: Builder(
                    builder: (context) => TextButton(
                      onPressed: () => showAppearancePicker(context),
                      child: const Text('Appearance'),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    await tester.tap(find.text('Appearance'));
    await tester.pumpAndSettle();
    expect(find.text('Follow System'), findsOneWidget);
    expect(find.text('Light Mode'), findsOneWidget);
    expect(find.text('Dark Mode'), findsOneWidget);
    await tester.tap(find.text('Dark Mode'));
    await tester.pumpAndSettle();
    expect(controller.themeMode, ThemeMode.dark);
    expect(saved, ['dark']);
    expect(tester.takeException(), isNull);
  });

  testWidgets('long label fits a narrow row and honors styles across themes', (
    tester,
  ) async {
    const longLabel =
        'A very long settings label that must wrap beside the value';
    const customLabel = TextStyle(color: Colors.purple, fontSize: 17);
    const customDescription = TextStyle(color: Colors.orange, fontSize: 11);

    Widget app(ThemeData theme, {bool custom = false}) => MaterialApp(
      theme: theme,
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 288,
            child: LabelTile(
              label: longLabel,
              desc: 'Description',
              labelStyle: custom ? customLabel : null,
              descStyle: custom ? customDescription : null,
              prefix: const Padding(
                padding: EdgeInsets.only(right: 12),
                child: Icon(Icons.map_outlined, size: 20),
              ),
              trailing: const LabelTileContent(
                content: 'Value',
                showArrow: true,
              ),
            ),
          ),
        ),
      ),
    );

    for (final theme in [AppTheme.light, AppTheme.dark]) {
      await tester.pumpWidget(app(theme));
      await tester.pumpAndSettle();
      final text = tester.widget<Text>(find.text(longLabel));
      expect(text.maxLines, 2);
      expect(text.style!.color, theme.extension<AppColors>()!.inkColor);
      expect(
        tester.widget<Text>(find.text('Description')).style!.color,
        theme.extension<AppColors>()!.mutedInkColor,
      );
      expect(tester.takeException(), isNull);

      await tester.pumpWidget(app(theme, custom: true));
      await tester.pumpAndSettle();
      expect(tester.widget<Text>(find.text(longLabel)).style, customLabel);
      expect(
        tester.widget<Text>(find.text('Description')).style,
        customDescription,
      );
      expect(tester.takeException(), isNull);
    }
  });
}

class _SettingsApi extends Fake implements RustLibApi {
  @override
  String crateApiApiShortCommitHash() => 'abc1234';
}
