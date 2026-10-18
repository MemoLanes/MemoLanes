import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/duo_hardware_test_data.dart';

import 'package:memolanes/body/journey/journey_info_form.dart';
import 'package:memolanes/body/journey/journey_info_fields.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/src/rust/import_data.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/theme/app_theme.dart';

const _fold = DuoHardware(
  supported: true,
  hasHinge: true,
  status: DuoHingeStatus.fullyOpen,
  angle: 180,
  folds: [],
);

void main() {
  testWidgets(
    'shared form preserves its draft and metadata across device changes',
    (tester) async {
      final scenes = ValueNotifier(
        const _Scene(
          supported: false,
          size: Size(320, 852),
          edge: DuoBarEdge.unknown,
        ),
      );
      addTearDown(scenes.dispose);
      JourneyInfo? saved;
      ImportPreprocessor? selectedPreprocessor;
      var infoTaps = 0;
      await _pumpForm(
        tester,
        scenes,
        '',
        onSave: (value) => saved = value,
        onPreprocessorInfo: () => infoTaps++,
        onPreprocessorSelected: (value) => selectedPreprocessor = value,
      );
      await tester.tap(find.byIcon(Icons.info_outline));
      await tester.pumpAndSettle();
      expect(infoTaps, 1);
      expect(find.text(tr('import.preprocessor.none')), findsNothing);
      await tester.tap(find.text(tr('import.preprocessor.generic')));
      await tester.pumpAndSettle();
      await tester.tap(find.text(tr('import.preprocessor.flight_track')));
      await tester.pumpAndSettle();
      expect(selectedPreprocessor, ImportPreprocessor.flightTrack);
      expect(infoTaps, 1);
      const note = 'A long imported note kept while folding and resizing.';
      await tester.enterText(find.byType(TextField), note);
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      for (final scene in [
        const _Scene(),
        const _Scene(book: true),
        const _Scene(size: Size(669, 951), edge: DuoBarEdge.horizontal),
        const _Scene(book: true, keyboard: 280),
        const _Scene(size: Size(678, 466), edge: DuoBarEdge.left),
      ]) {
        tester.view.physicalSize = scene.size;
        scenes.value = scene;
        await tester.pumpAndSettle();
        expect(
          tester.widget<TextField>(find.byType(TextField)).controller!.text,
          note,
        );
      }
      await tester.ensureVisible(find.text(tr('common.save')));
      await tester.pumpAndSettle();
      await tester.tap(find.text(tr('common.save')));
      await tester.pumpAndSettle();
      expect(saved!.note, note);
      expect(saved!.journeyKind, JourneyKind.flight);
      expect(saved!.journeyDate, DateTime.utc(2026, 10, 1));
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets(
    'a landscape form clears the keyboard and saves its draft',
    (tester) async {
      final scenes = ValueNotifier(
        const _Scene(
          supported: false,
          edge: DuoBarEdge.unknown,
          size: Size(667, 375),
          keyboard: 200,
        ),
      );
      addTearDown(scenes.dispose);
      JourneyInfo? saved;
      await _pumpForm(
        tester,
        scenes,
        'Original note',
        onSave: (value) => saved = value,
      );
      final card = tester.getRect(find.byType(JourneyInfoPanelSurface));
      expect(
        card.bottom,
        lessThanOrEqualTo(scenes.value.size.height - scenes.value.keyboard),
      );
      await tester.ensureVisible(find.byType(TextField));
      await tester.enterText(find.byType(TextField), 'Keyboard draft');
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text(tr('common.save')));
      await tester.pumpAndSettle();
      expect(find.text(tr('common.save')).hitTestable(), findsOneWidget);
      await tester.tap(find.text(tr('common.save')));
      await tester.pumpAndSettle();
      expect(saved!.note, 'Keyboard draft');
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.iOS,
      TargetPlatform.android,
    }),
  );
}

Future<void> _pumpForm(
  WidgetTester tester,
  ValueNotifier<_Scene> scenes,
  String note, {
  required ValueChanged<JourneyInfo> onSave,
  VoidCallback? onPreprocessorInfo,
  ValueChanged<ImportPreprocessor>? onPreprocessorSelected,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = scenes.value.size;
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
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
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: const [Locale('en', 'US')],
        path: 'assets/translations',
        assetLoader: const AppTranslationLoader(),
        startLocale: const Locale('en', 'US'),
        saveLocale: false,
        child: Builder(
          builder: (context) => MaterialApp(
            theme: AppTheme.light,
            localizationsDelegates: [
              ...GlobalMaterialLocalizations.delegates,
              ...context.localizationDelegates,
            ],
            supportedLocales: context.supportedLocales,
            locale: context.locale,
            builder: (_, child) => ValueListenableBuilder<_Scene>(
              valueListenable: scenes,
              child: child,
              builder: (_, scene, child) => MediaQuery(
                data: scene.media,
                child: DuoHardwareScope(
                  hardware: scene.fold,
                  child: DuoBarScope(edge: scene.edge, child: child!),
                ),
              ),
            ),
            home: Scaffold(
              resizeToAvoidBottomInset: false,
              body: AppLayoutBuilder(
                builder: (_, layout) => Stack(
                  children: [
                    MapOverlayPanel.detail(
                      child: JourneyInfoPanelSurface(
                        child: SingleChildScrollView(
                          padding: const EdgeInsets.only(top: 10),
                          child: JourneyInfoForm(
                            startTime: null,
                            endTime: null,
                            journeyDate: SimpleDate(2026, 10, 1),
                            note: note,
                            onSave: onSave,
                            journeyKind: JourneyKind.flight,
                            extraFields: ImportPreprocessorTile(
                              compact: true,
                              value: ImportPreprocessor.generic,
                              onInfoTap: onPreprocessorInfo ?? () {},
                              onSelected: onPreprocessorSelected ?? (_) {},
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
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
}

class _Scene {
  const _Scene({
    this.size = const Size(951, 669),
    this.edge = DuoBarEdge.right,
    this.supported = true,
    this.book = false,
    this.keyboard = 0,
  });

  final Size size;
  final DuoBarEdge edge;
  final bool supported;
  final bool book;
  final double keyboard;

  EdgeInsets get padding => EdgeInsets.only(
    left: edge == DuoBarEdge.left ? 84 : 0,
    right: edge == DuoBarEdge.right ? 84 : 0,
    bottom: 34,
  );
  MediaQueryData get media => MediaQueryData(
    size: size,
    padding: padding.copyWith(bottom: keyboard > 0 ? 0 : 34),
    viewPadding: padding,
    viewInsets: EdgeInsets.only(bottom: keyboard),
  );
  DuoHardware get fold => supported
      ? _fold.copyWith(
          status: book
              ? DuoHingeStatus.partiallyOpen
              : DuoHingeStatus.fullyOpen,
          folds: [
            if (book)
              DuoRegion(
                Rect.fromLTWH(size.width / 2 - 20, 0, 40, size.height),
                isActive: true,
              ),
          ],
          cameras: [
            if (edge != DuoBarEdge.horizontal)
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
