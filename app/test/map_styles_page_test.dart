import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/settings/map_styles_page.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('en', 'US');

  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
  });

  testWidgets('add, edit and delete a raster map style in a narrow viewport', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(320, 700);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final values = <String, String>{};
    final store = MapStyleStore(
      readString: (key) => values[key],
      writeString: (key, value) {
        if (value != null) values[key] = value;
        return true;
      },
    );
    final app = EasyLocalization(
      supportedLocales: const [locale],
      path: 'assets/translations',
      assetLoader: const AppTranslationLoader(),
      fallbackLocale: locale,
      child: Builder(
        builder: (context) => MaterialApp(
          theme: AppTheme.light,
          localizationsDelegates: [
            ...GlobalMaterialLocalizations.delegates,
            ...context.localizationDelegates,
          ],
          supportedLocales: context.supportedLocales,
          locale: context.locale,
          home: MapStylesPage(store: store),
        ),
      ),
    );
    await tester.runAsync(() async {
      await const AppTranslationLoader().load('assets/translations', locale);
      await tester.pumpWidget(app);
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    Future<void> tapVisible(Finder finder) async {
      await tester.ensureVisible(finder);
      await tester.tap(finder);
      await tester.pumpAndSettle();
    }

    await tapVisible(find.text('Add style'));
    await tester.enterText(find.byType(TextFormField).at(0), 'My tiles');
    await tapVisible(find.text('Image tiles'));
    await tester.enterText(
      find.byType(TextFormField).at(1),
      'https://example.com/tiles.png',
    );
    await tapVisible(find.text('Save and use'));
    expect(
      find.text('The tile URL must include {z}, {x}, and {y}'),
      findsOneWidget,
    );
    expect(store.customStyles, isEmpty);

    const url = 'https://example.com/{z}/{x}/{y}.png?key=user-key&language=zh';
    await tester.enterText(find.byType(TextFormField).at(1), url);
    await tapVisible(find.text('Tile settings'));
    await tapVisible(find.text('256 px'));
    await tapVisible(find.text('512 px').last);
    await tapVisible(find.text('XYZ'));
    await tapVisible(find.text('TMS').last);
    await tapVisible(find.text('Save and use'));
    expect(find.text('My tiles'), findsOneWidget);
    final id = store.selected.id;
    expect(store.selected.url, url);
    expect(store.selected.tileSize, 512);
    expect(store.selected.scheme, MapTileScheme.tms);

    // A failure reported by an already mounted map updates this open settings
    // page through the store, without navigating away and rebuilding it.
    store.recordFallback(store.selected);
    await tester.pumpAndSettle();
    final defaultOption = find.ancestor(
      of: find.text('OpenFreeMap'),
      matching: find.byType(AppOptionTile),
    );
    expect(tester.widget<AppOptionTile>(defaultOption).selected, isTrue);
    await tapVisible(find.byTooltip('Edit style'));
    expect(store.selected.id, MapStyle.all.first.id);
    await tester.enterText(find.byType(TextFormField).at(0), 'Renamed map');
    await tapVisible(find.text('Save'));
    expect(find.text('Renamed map'), findsOneWidget);
    expect(store.customStyles, hasLength(1));
    expect(store.selected.id, MapStyle.all.first.id);
    await tapVisible(find.text('Renamed map'));
    expect(store.selected.id, id);

    await tapVisible(find.text('View current style attribution'));
    expect(
      tester.widget<MarkdownBody>(find.byType(MarkdownBody)).data,
      '**Renamed map**',
    );
    Navigator.of(tester.element(find.byType(MarkdownBody))).pop();
    await tester.pumpAndSettle();

    await tapVisible(find.byTooltip('Edit style'));
    await tapVisible(find.text('Delete style'));
    expect(
      find.textContaining('will switch to the default style'),
      findsOneWidget,
    );
    await tapVisible(find.text('Delete style').hitTestable().last);
    expect(store.customStyles, isEmpty);
    expect(store.selected.id, MapStyle.all.first.id);
    expect(find.text('Renamed map'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
