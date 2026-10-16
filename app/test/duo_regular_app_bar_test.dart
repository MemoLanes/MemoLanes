import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/duo_hardware_test_data.dart';

import 'package:memolanes/body/settings/vector_multi_import_page.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/back_button_target.dart';
import 'package:memolanes/common/component/app_checkbox.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;

const _duo = DuoHardware(
  supported: true,
  hasHinge: true,
  status: DuoHingeStatus.fullyOpen,
  angle: 180,
  folds: [],
);

void main() {
  testWidgets('import selections and note survive native layout changes', (
    tester,
  ) async {
    final ordinary = _Scene(DuoBarEdge.unknown, supported: false);
    _sizeView(tester, ordinary.size);
    final scenes = ValueNotifier(ordinary);
    addTearDown(scenes.dispose);
    await _pumpVector(tester, scenes);
    final noteField = find.byType(TextField);
    await tester.enterText(noteField, 'kept import note');
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pumpAndSettle();
    final row = find.widgetWithText(LabelTile, '2026-10-01');
    final checkbox = find.descendant(
      of: row,
      matching: find.byType(AppCheckbox),
    );
    final all = find.widgetWithText(
      TextButton,
      tr('import.journey_selection.deselect_all'),
    );
    await tester.tap(all);
    await tester.pumpAndSettle();
    expect(tester.widget<AppCheckbox>(checkbox).value, isFalse);
    expect(tester.takeException(), isNull);

    for (final next in [
      _Scene(DuoBarEdge.right),
      _Scene(DuoBarEdge.left),
      _Scene(DuoBarEdge.horizontal),
      ordinary,
    ]) {
      scenes.value = next;
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(noteField).controller!.text,
        'kept import note',
      );
      expect(tester.widget<AppCheckbox>(checkbox).value, isFalse);
      expect(
        find.widgetWithText(
          TextButton,
          tr('import.journey_selection.select_all'),
        ),
        findsOneWidget,
      );
      if (next.supported) {
        await tester.ensureVisible(row);
        await tester.pumpAndSettle();
        expect(tester.getRect(row).overlaps(tester.getRect(_target)), isFalse);
      }
      expect(tester.takeException(), isNull);
    }
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets(
    'only the current page owns Back during push, dialog, pop and disposal',
    (tester) async {
      final scene = _Scene(DuoBarEdge.right);
      _sizeView(tester, scene.size);
      final scenes = ValueNotifier(scene);
      addTearDown(scenes.dispose);
      final navigator = GlobalKey<NavigatorState>();
      final canPop = ValueNotifier(false);
      addTearDown(canPop.dispose);
      var attempts = 0;
      await tester.pumpWidget(
        _app(
          scenes,
          navigator: navigator,
          home: const Scaffold(body: Center(child: Text('Home'))),
        ),
      );
      navigator.currentState!.push(
        MaterialPageRoute<void>(
          builder: (_) => ValueListenableBuilder<bool>(
            valueListenable: canPop,
            builder: (_, allowed, _) => PopScope(
              canPop: allowed,
              onPopInvokedWithResult: (_, _) => attempts++,
              child: _page(),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(_target, findsOneWidget);
      navigator.currentState!.push(
        MaterialPageRoute<void>(builder: (_) => _page(title: 'Second')),
      );
      await tester.pump();
      await tester.pump();
      expect(_target, findsOneWidget);
      navigator.currentState!.pop();
      await tester.pump(const Duration(milliseconds: 10));
      await tester.pump();
      expect(_target, findsOneWidget);
      await tester.pumpAndSettle();
      showAppDialog<void>(
        tester.element(find.byType(CapsuleStyleAppBar)),
        builder: (_) =>
            const AppDialogContent(title: 'Dialog', child: Text('Modal')),
      );
      await tester.pumpAndSettle();
      expect(_targets, findsNothing);
      scenes.value = _Scene(DuoBarEdge.left);
      await tester.pumpAndSettle();
      expect(_targets, findsNothing);
      expect(find.text('Modal'), findsOneWidget);
      navigator.currentState!.pop();
      await tester.pumpAndSettle();
      expect(_target, findsOneWidget);
      await tester.tapAt(tester.getRect(_target).center);
      await tester.pumpAndSettle();
      expect(find.text('Settings'), findsOneWidget);
      expect(attempts, 1);
      canPop.value = true;
      await tester.pumpAndSettle();
      await tester.tapAt(tester.getRect(_target).center);
      await tester.pumpAndSettle();
      expect(find.text('Home'), findsOneWidget);
      expect(_targets, findsNothing);
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );
}

Future<void> _pumpVector(
  WidgetTester tester,
  ValueNotifier<_Scene> scenes,
) async {
  // EasyLocalization reads preferences during initialization. Mock its
  // platform boundary without adding a transitive package import to this test.
  const preferences = MethodChannel('plugins.flutter.io/shared_preferences');
  final messenger = tester.binding.defaultBinaryMessenger;
  messenger.setMockMethodCallHandler(
    preferences,
    (call) async => call.method == 'getAll' ? <String, Object>{} : null,
  );
  addTearDown(() => messenger.setMockMethodCallHandler(preferences, null));
  await tester.runAsync(() async {
    await EasyLocalization.ensureInitialized();
    await const AppTranslationLoader().load(
      'assets/translations',
      const Locale('en', 'US'),
    );
    await tester.pumpWidget(_vectorApp(scenes));
    await tester.pump(const Duration(seconds: 1));
  });
  await tester.pumpAndSettle();
}

Widget _vectorApp(ValueNotifier<_Scene> scenes) => EasyLocalization(
  supportedLocales: const [Locale('en', 'US')],
  path: 'assets/translations',
  assetLoader: const AppTranslationLoader(),
  startLocale: const Locale('en', 'US'),
  saveLocale: false,
  child: Builder(
    builder: (context) => MaterialApp(
      theme: AppTheme.light,
      localizationsDelegates: context.localizationDelegates,
      supportedLocales: context.supportedLocales,
      locale: context.locale,
      builder: (context, child) => ValueListenableBuilder<_Scene>(
        valueListenable: scenes,
        child: child,
        builder: (context, scene, child) => MediaQuery(
          data: scene.media,
          child: DuoHardwareScope(
            hardware: scene.fold,
            child: DuoBarScope(edge: scene.edge, child: child!),
          ),
        ),
      ),
      home: VectorMultiImportPage(
        vectorData: _UnusedVectorData(),
        initialPreprocessor: import_api.ImportPreprocessor.generic,
        parts: [
          for (var i = 1; i <= 18; i++)
            import_api.VectorImportPartSummary(
              journeyDate: '2026-10-${i.toString().padLeft(2, '0')}',
              pointCount: BigInt.from(100),
              missingTimestampCount: BigInt.zero,
            ),
        ],
      ),
    ),
  ),
);

// Selection/layout does not call the Rust importer or preview APIs.
class _UnusedVectorData extends Fake implements import_api.RawVectorData {}

final _targets = find.byType(BackButtonTarget);
Finder get _target => _targets.first;

void _sizeView(WidgetTester tester, Size size) {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
}

Widget _page({String title = 'Settings'}) => Scaffold(
  appBar: CapsuleStyleAppBar(title: title),
  body: const Center(child: Text('Body')),
);

Widget _app(
  ValueNotifier<_Scene> scenes, {
  required Widget home,
  GlobalKey<NavigatorState>? navigator,
}) => MaterialApp(
  navigatorKey: navigator,
  theme: AppTheme.light,
  builder: (context, child) => ValueListenableBuilder<_Scene>(
    valueListenable: scenes,
    child: child,
    builder: (context, scene, child) => MediaQuery(
      data: scene.media,
      child: DuoHardwareScope(
        hardware: scene.fold,
        child: DuoBarScope(edge: scene.edge, child: child!),
      ),
    ),
  ),
  home: home,
);

class _Scene {
  _Scene(this.edge, {this.supported = true});
  final DuoBarEdge edge;
  final Size size = const Size(951, 669);
  final bool supported;
  EdgeInsets get padding => !supported
      ? const EdgeInsets.fromLTRB(3, 59, 5, 34)
      : EdgeInsets.fromLTRB(
          edge == DuoBarEdge.left ? 84 : 0,
          0,
          edge == DuoBarEdge.right ? 84 : 0,
          34,
        );
  MediaQueryData get media =>
      MediaQueryData(size: size, padding: padding, viewPadding: padding);
  DuoHardware get fold => supported
      ? _duo.copyWith(
          cameras: [
            if (edge == DuoBarEdge.left || edge == DuoBarEdge.right)
              DuoRegion(
                Rect.fromLTWH(
                  edge == DuoBarEdge.left ? 0 : size.width - 84,
                  0,
                  84,
                  120,
                ),
                isActive: true,
              ),
          ],
        )
      : DuoHardware.none;
}
