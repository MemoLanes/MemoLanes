import 'dart:async';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/journey_export.dart';
import 'package:memolanes/body/journey/list/journey_layer_filter_menu.dart';
import 'package:memolanes/body/map/overlay/journey_detail_card.dart';
import 'package:memolanes/body/map/overlay/journey_more_dialog.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('en', 'US');
  const loader = AppTranslationLoader();

  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    // Use the SDK's real font metrics for narrow-screen layout assertions.
    final artifacts = File(Platform.resolvedExecutable).parent.parent.parent;
    final font = FontLoader('Roboto')
      ..addFont(
        File.fromUri(artifacts.uri.resolve('material_fonts/Roboto-Regular.ttf'))
            .readAsBytes()
            .then((bytes) => ByteData.sublistView(bytes)),
      );
    await font.load();
    await EasyLocalization.ensureInitialized();
    await loader.load('assets/translations', locale);
  });

  Future<void> pumpApp(WidgetTester tester, Widget home) async {
    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale],
          path: 'assets/translations',
          assetLoader: loader,
          fallbackLocale: locale,
          child: Builder(
            builder: (context) => MaterialApp(
              theme: ThemeData(fontFamily: 'Roboto'),
              localizationsDelegates: context.localizationDelegates,
              supportedLocales: context.supportedLocales,
              locale: context.locale,
              home: home,
            ),
          ),
        ),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();
  }

  final journey = JourneyHeader(
    id: 'original',
    revision: 'revision',
    journeyDate: DateTime.utc(2023, 11, 14),
    createdAt: DateTime.utc(2023, 11, 14),
    journeyType: JourneyType.vector,
    journeyKind: JourneyKind.defaultKind,
    note: 'Original note',
    hasRawData: false,
  );

  final journeyWithRawData = JourneyHeader(
    id: 'with-raw-data',
    revision: 'revision',
    journeyDate: DateTime.utc(2023, 11, 14),
    createdAt: DateTime.utc(2023, 11, 14),
    journeyType: JourneyType.vector,
    journeyKind: JourneyKind.defaultKind,
    hasRawData: true,
  );

  setUpAll(() => RustLib.initMock(api: _JourneyListApi(journey)));
  tearDownAll(RustLib.dispose);

  testWidgets('journey information indicates attached raw data', (
    tester,
  ) async {
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailCard(
          journey: journeyWithRawData,
          isEditing: false,
          onExport: () {},
          onEdit: () {},
          onMore: () {},
          onSave: (_) async {},
        ),
      ),
    );

    expect(find.text('Raw Data'), findsOneWidget);
    expect(find.text('Included'), findsOneWidget);
  });

  testWidgets('journey without raw data has no raw-data export switch', (
    tester,
  ) async {
    await pumpApp(
      tester,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () =>
                showJourneyExportPicker(context, journey, hasRawData: false),
            child: const Text('Open Export'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open Export'));
    await tester.pumpAndSettle();
    expect(find.byType(Switch), findsNothing);
    expect(find.text('Preserve raw location data'), findsNothing);
  });

  testWidgets('raw-data export choice survives format changes', (tester) async {
    await pumpApp(
      tester,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () => showJourneyExportPicker(
              context,
              journeyWithRawData,
              hasRawData: true,
            ),
            child: const Text('Open Export'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open Export'));
    await tester.pumpAndSettle();
    expect(tester.widget<Switch>(find.byType(Switch)).value, isTrue);

    await tester.ensureVisible(find.byType(Switch));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Fog of World snapshot (*.fwss)'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('MemoLanes standard format (*.mldx)'));
    await tester.pumpAndSettle();

    expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);

    await tester.ensureVisible(find.text('Raw location data'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Raw location data'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Journey data'));
    await tester.pumpAndSettle();

    expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);
  });

  for (final width in [320.0, 360.0]) {
    testWidgets('compact journey picker fits a $width px screen', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = Size(width, 640);
      addTearDown(tester.view.reset);

      await pumpApp(tester, const Scaffold(body: JourneyOverlay()));
      expect(tester.takeException(), isNull);
      expect(find.text('Nov'), findsOneWidget);
      final filter = find.byIcon(Icons.layers_outlined);
      expect(filter.hitTestable(), findsOneWidget);
      expect(
        tester
            .widget<Tooltip>(
              find.ancestor(of: filter, matching: find.byType(Tooltip)),
            )
            .message,
        'All',
      );
      await tester.tap(filter);
      await tester.pumpAndSettle();
      expect(find.byType(JourneyLayerFilterMenu), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('detail form scrolls above a landscape keyboard', (tester) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(667, 375);
    tester.view.viewInsets = const FakeViewPadding(bottom: 200);
    addTearDown(tester.view.reset);
    final saved = <JourneyInfo>[];

    await pumpApp(
      tester,
      Scaffold(
        body: Stack(
          fit: StackFit.expand,
          children: [
            Positioned(
              left: 16,
              right: 16,
              top: 68,
              bottom: 16,
              child: Align(
                alignment: Alignment.bottomCenter,
                child: SizedBox(
                  width: 430,
                  child: CollapsibleJourneyDetail(
                    child: JourneyDetailCard(
                      journey: journey,
                      isEditing: true,
                      onExport: () {},
                      onEdit: () {},
                      onMore: () {},
                      onSave: (info) async => saved.add(info),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );

    final card = find.byType(CollapsibleJourneyDetail);
    final viewport = tester.getRect(card);
    expect(viewport.top, greaterThanOrEqualTo(68));
    expect(viewport.bottom, lessThanOrEqualTo(159));
    final handle = find.byKey(const ValueKey('journey-detail-drag-handle'));
    final handlePosition = tester.getTopLeft(handle);

    await tester.ensureVisible(find.byType(TextField));
    await tester.enterText(find.byType(TextField), 'Landscape draft');
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Journey Date'));
    await tester.pumpAndSettle();
    expect(find.text('Journey Date').hitTestable(), findsOneWidget);
    await tester.ensureVisible(find.text('Save'));
    await tester.pumpAndSettle();
    expect(find.text('Save').hitTestable(), findsOneWidget);
    expect(tester.getTopLeft(handle), handlePosition);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved.single.note, 'Landscape draft');
    expect(tester.takeException(), isNull);
  });

  testWidgets('saving edits submits metadata and waits for completion', (
    tester,
  ) async {
    final saved = <JourneyInfo>[];
    final saveFinished = Completer<void>();
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailCard(
          journey: journey,
          isEditing: true,
          onExport: () {},
          onEdit: () {},
          onMore: () {},
          onSave: (info) async {
            saved.add(info);
            await saveFinished.future;
          },
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Changed note');
    await tester.tap(find.text('Save'));
    await tester.pump();
    expect(tester.widget<TextField>(find.byType(TextField)).readOnly, isTrue);
    await tester.tap(find.text('Save'));
    expect(saved, hasLength(1));
    saveFinished.complete();
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.note, 'Changed note');
    expect(saved.single.journeyDate, journey.journeyDate);
    expect(saved.single.journeyKind, journey.journeyKind);
    expect(tester.widget<TextField>(find.byType(TextField)).readOnly, isFalse);
  });

  testWidgets('failed save keeps the draft and allows retry', (tester) async {
    final saved = <JourneyInfo>[];
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailCard(
          journey: journey,
          isEditing: true,
          onExport: () {},
          onEdit: () {},
          onMore: () {},
          onSave: (info) async {
            saved.add(info);
            if (saved.length == 1) throw StateError('Storage unavailable');
          },
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Keep my draft');
    await tester.tap(find.text('Save'));
    // The save button remains busy until the error dialog is dismissed.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(
      find.text('Something went wrong. Please try again.'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
    invokeConfirmationAction(tester, 'OK');
    await tester.pumpAndSettle();
    expect(find.text('Keep my draft'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).readOnly, isFalse);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved, hasLength(2));
    expect(saved[1].note, saved[0].note);
    expect(find.byType(CommonDialog), findsNothing);
  });

  testWidgets('collapsing and restoring details preserves the unsaved draft', (
    tester,
  ) async {
    // Exercise the gestures without device-only haptics or MMKV setup.
    AppHaptics.debugHapticsEnabledOverride = false;
    addTearDown(() => AppHaptics.debugHapticsEnabledOverride = null);
    final saved = <JourneyInfo>[];
    await pumpApp(
      tester,
      Scaffold(
        body: Align(
          alignment: Alignment.bottomCenter,
          child: SizedBox(
            width: 430,
            child: CollapsibleJourneyDetail(
              child: JourneyDetailCard(
                journey: journey,
                isEditing: true,
                onExport: () {},
                onEdit: () {},
                onMore: () {},
                onSave: (info) async => saved.add(info),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Unsaved draft');
    await tester.drag(
      find.byKey(const ValueKey('journey-detail-drag-handle')),
      const Offset(0, 100),
    );
    // Wait beyond the transition: AnimatedSwitcher used to dispose the draft
    // only after the outgoing card finished animating.
    await tester.pumpAndSettle();
    expect(find.byType(TextField).hitTestable(), findsNothing);
    expect(find.text('Save').hitTestable(), findsNothing);
    expect(tester.testTextInput.isVisible, isFalse);
    expect(saved, isEmpty);

    await tester.tap(find.byIcon(Icons.keyboard_arrow_up_rounded));
    await tester.pumpAndSettle();
    expect(find.text('Unsaved draft'), findsOneWidget);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.note, 'Unsaved draft');
  });

  testWidgets('more ignores duplicate actions during navigation', (
    tester,
  ) async {
    final results = <JourneyMoreAction?>[];
    await pumpApp(
      tester,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () async =>
                results.add(await showJourneyMoreDialog(context)),
            child: const Text('Open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    final delete = tester
        .widget<AppButton>(
          find.ancestor(
            of: find.text('Delete Journey'),
            matching: find.byType(AppButton),
          ),
        )
        .onPressed!;
    // Invoke stale callbacks before a frame is drawn: a confirmation must not
    // stack twice.
    delete();
    delete();
    await tester.pumpAndSettle();
    expect(find.byType(CommonDialog), findsOneWidget);
    expect(results, isEmpty);
    final cancel = tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .singleWhere((button) => button.text == 'Cancel')
        .onPressed;
    cancel();
    cancel();
    await tester.pumpAndSettle();
    expect(find.text('Delete Journey').hitTestable(), findsOneWidget);
    expect(results, isEmpty);
    delete();
    await tester.pumpAndSettle();
    final confirm = tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .singleWhere((button) => button.text == 'Delete')
        .onPressed;
    confirm();
    confirm();
    await tester.pumpAndSettle();
    expect(results, [JourneyMoreAction.delete]);
    expect(find.text('Open').hitTestable(), findsOneWidget);
    expect(find.byType(Dialog), findsNothing);
  });

  testWidgets('cancel deletion returns to more and allows retrying', (
    tester,
  ) async {
    final results = <JourneyMoreAction?>[];
    await pumpApp(
      tester,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () async =>
                results.add(await showJourneyMoreDialog(context)),
            child: const Text('Open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    // Both Cancel and system Back must leave the menu open without a result.
    for (final cancelWithBack in [false, true]) {
      await tester.tap(find.text('Delete Journey'));
      await tester.pumpAndSettle();
      expect(find.text('Delete this journey record?'), findsOneWidget);
      if (cancelWithBack) {
        await tester.binding.handlePopRoute();
      } else {
        invokeConfirmationAction(tester, 'Cancel');
      }
      await tester.pumpAndSettle();
      expect(find.text('Delete this journey record?'), findsNothing);
      expect(find.text('Delete Journey').hitTestable(), findsOneWidget);
      expect(results, isEmpty);
    }
    await tester.tap(find.text('Delete Journey'));
    await tester.pumpAndSettle();
    invokeConfirmationAction(tester, 'Delete');
    await tester.pumpAndSettle();
    expect(results, [JourneyMoreAction.delete]);
    expect(find.byType(Dialog), findsNothing);
  });
}

void invokeConfirmationAction(WidgetTester tester, String label) {
  expect(find.text(label).hitTestable(), findsOneWidget);
  // Exercise the real dialog navigation callback without requiring the
  // device-only MMKV/haptic setup used by CommonDialog's tap wrapper.
  tester
      .widget<CommonDialog>(find.byType(CommonDialog))
      .buttons
      .singleWhere((button) => button.text == label)
      .onPressed();
}

class _JourneyListApi extends Fake implements RustLibApi {
  _JourneyListApi(this.journey);

  final JourneyHeader journey;

  @override
  Future<DateTime?> crateApiApiEarliestJourneyDate() async =>
      journey.journeyDate;

  @override
  Future<List<DateTime>> crateApiApiJourneyDates({
    Set<JourneyKind>? journeyKinds,
  }) async => [journey.journeyDate];

  @override
  Future<List<JourneyHeader>> crateApiApiListJourneysOnDate({
    required int year,
    required int month,
    required int day,
    Set<JourneyKind>? journeyKinds,
  }) async => [journey];
}
