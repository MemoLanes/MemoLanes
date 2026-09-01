import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/editor/journey_track_edit_mode_bar.dart';
import 'package:memolanes/common/app_translation_loader.dart';

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
    await const AppTranslationLoader().load('assets/translations', locale);
  });

  Widget buildApp({VoidCallback? onUndo, VoidCallback? onSave}) {
    return EasyLocalization(
      supportedLocales: const [locale],
      path: 'assets/translations',
      assetLoader: const AppTranslationLoader(),
      fallbackLocale: locale,
      child: Builder(
        builder: (context) => MaterialApp(
          localizationsDelegates: context.localizationDelegates,
          supportedLocales: context.supportedLocales,
          locale: context.locale,
          home: Scaffold(
            body: SafeArea(
              minimum: const EdgeInsets.all(ModeSwitchBar.safeAreaMinimum),
              child: ModeSwitchBar(
                currentMode: OperationMode.edit,
                onModeChanged: (_) {},
                currentDrawMode: DrawEntryMode.linked,
                isDrawMenuOpen: true,
                onDrawPressed: () {},
                onDrawModeChanged: (_) {},
                onUndo: onUndo,
                onSave: onSave,
              ),
            ),
          ),
        ),
      ),
    );
  }

  testWidgets('editor controls fit a narrow viewport with the draw menu open', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 480);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.runAsync(() async {
      await tester.pumpWidget(buildApp(onUndo: () {}, onSave: () {}));
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byKey(const ValueKey('draw-mode-menu')), findsOneWidget);
    expect(find.byIcon(Icons.undo_rounded), findsOneWidget);
    expect(find.byIcon(Icons.check_rounded), findsWidgets);
  });

  testWidgets('undo and save are disabled when callbacks are absent', (
    tester,
  ) async {
    await tester.runAsync(() async {
      await tester.pumpWidget(buildApp());
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    final undoButton = tester.widget<IconButton>(
      find.ancestor(
        of: find.byIcon(Icons.undo_rounded),
        matching: find.byType(IconButton),
      ),
    );
    final saveButton = tester.widget<FilledButton>(find.byType(FilledButton));

    expect(undoButton.onPressed, isNull);
    expect(saveButton.onPressed, isNull);
  });
}
