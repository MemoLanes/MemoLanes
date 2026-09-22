import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/list/journey_layer_filter_menu.dart';
import 'package:memolanes/body/journey/list/journey_list_calendar.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/body/map/overlay/journey_detail_card.dart';
import 'package:memolanes/body/map/overlay/journey_detail_overlay.dart';
import 'package:memolanes/body/map/overlay/journey_more_dialog.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('en', 'US');
  const loader = AppTranslationLoader();
  final originalWakelockPlatform = wakelockPlusPlatformInstance;

  setUpAll(() async {
    wakelockPlusPlatformInstance = _TestWakelockPlatform();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
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
              theme: AppTheme.light,
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
  );
  final mockApi = _JourneyListApi(journey);

  setUpAll(() => RustLib.initMock(api: mockApi));
  tearDownAll(() {
    RustLib.dispose();
    wakelockPlusPlatformInstance = originalWakelockPlatform;
  });

  testWidgets('journey layer filter opens its menu', (tester) async {
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyOverlay(
          onJourneySelected: (_) async {},
          isLoading: false,
          refreshRevision: 0,
        ),
      ),
    );
    final filter = find.byTooltip('All');
    expect(filter.hitTestable(), findsOneWidget);
    await tester.tap(filter);
    await tester.pumpAndSettle();
    expect(find.byType(JourneyLayerFilterMenu), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('journey calendar keeps its month after closing details', (
    tester,
  ) async {
    var showingDetail = false;
    var revision = 0;
    late StateSetter updateHost;
    await pumpApp(
      tester,
      StatefulBuilder(
        builder: (context, setState) {
          updateHost = setState;
          return Scaffold(
            body: JourneyOverlay(
              onJourneySelected: (_) async {},
              isLoading: false,
              refreshRevision: revision,
              detail: showingDetail
                  ? const ColoredBox(color: Colors.transparent)
                  : null,
            ),
          );
        },
      ),
    );

    final controller = tester
        .widget<JourneyListCalendar>(find.byType(JourneyListCalendar))
        .controller;
    await controller.displayMonth(SimpleDate(2023, 12, 1));
    await tester.pumpAndSettle();

    updateHost(() => showingDetail = true);
    await tester.pumpAndSettle();
    updateHost(() {
      showingDetail = false;
      revision++;
    });
    await tester.pumpAndSettle();
    expect(find.text('Dec'), findsOneWidget);
  });

  for (final keepSelectedMonth in [false, true]) {
    testWidgets('calendar follows a changed earliest date with '
        '${keepSelectedMonth ? 'a retained' : 'a deleted'} selected month', (
      tester,
    ) async {
      final november = SimpleDate(2023, 11, 14);
      final february = SimpleDate(2024, 2, 1);
      final march = SimpleDate(2024, 3, 1);
      var earliest = november;
      var dates = [november, february, march];
      final controller = JourneyListController(
        earliestJourneyDateLoader: () async => earliest,
        journeyDatesLoader: (_) async => dates,
        journeyHeadersLoader: (_, _) async => [],
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.selectDate(keepSelectedMonth ? march : november);
      await pumpApp(
        tester,
        Scaffold(
          body: Center(
            child: SizedBox(
              width: 340,
              child: AnimatedBuilder(
                animation: controller,
                builder: (context, _) => JourneyListCalendar(
                  controller: controller,
                  firstDate: controller.firstDate!,
                ),
              ),
            ),
          ),
        ),
      );

      // Deleting November's last journey changes the calendar's page origin.
      earliest = february;
      dates = [february, march];
      await controller.refresh();
      await tester.pumpAndSettle();

      final expectedDate = keepSelectedMonth ? march : february;
      expect(controller.selectedDate, expectedDate);
      expect(find.text(keepSelectedMonth ? 'Mar' : 'Feb'), findsOneWidget);
      expect(find.text('29'), findsOneWidget);
      expect(
        find.text('31'),
        keepSelectedMonth ? findsOneWidget : findsNothing,
      );
      expect(
        find.text('30'),
        keepSelectedMonth ? findsOneWidget : findsNothing,
      );
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('cancel discards the draft without saving or closing details', (
    tester,
  ) async {
    mockApi.savedMetadata.clear();
    var closed = false;
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailOverlay(
          journey: journey,
          onClose: () => closed = true,
          onMapChanged: (_, _) {},
        ),
      ),
    );

    await tester.tap(find.text('Edit'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Edit Journey Information'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'Unsaved draft');
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();

    expect(closed, isFalse);
    expect(mockApi.savedMetadata, isEmpty);

    await tester.tap(find.text('Edit'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Edit Journey Information'));
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'Original note',
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('saving journey information requires confirmation', (
    tester,
  ) async {
    mockApi.savedMetadata.clear();
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailOverlay(
          journey: journey,
          onClose: () {},
          onMapChanged: (_, _) {},
        ),
      ),
    );

    await tester.tap(find.text('Edit'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Edit Journey Information'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Confirmed note');
    await tester.tap(find.text('Save'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    expect(mockApi.savedMetadata, isEmpty);
    invokeConfirmationAction(tester, 'Cancel');
    await tester.pumpAndSettle();
    expect(find.text('Confirmed note'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    expect(mockApi.savedMetadata, isEmpty);

    await tester.tap(find.text('Save'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    invokeConfirmationAction(tester, 'OK');
    await tester.pumpAndSettle();
    expect(mockApi.savedMetadata.single.note, 'Confirmed note');
    expect(find.byType(TextField), findsNothing);
    expect(find.text('Confirmed note'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'home back cancels editing, then closes details and unregisters',
    (tester) async {
      mockApi.savedMetadata.clear();
      var closed = false;
      var showingDetail = true;
      late StateSetter updateHost;

      await pumpApp(
        tester,
        StatefulBuilder(
          builder: (context, setState) {
            updateHost = setState;
            return Scaffold(
              body: showingDetail
                  ? JourneyDetailOverlay(
                      journey: journey,
                      onClose: () {
                        closed = true;
                        updateHost(() => showingDetail = false);
                      },
                      onMapChanged: (_, _) {},
                    )
                  : const SizedBox.expand(),
            );
          },
        ),
      );

      await tester.tap(find.text('Edit'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Edit Journey Information'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Unsaved draft');

      expect(handleHomeBack(), isTrue);
      await tester.pumpAndSettle();
      expect(closed, isFalse);
      expect(find.byType(TextField), findsNothing);
      expect(mockApi.savedMetadata, isEmpty);

      expect(handleHomeBack(), isTrue);
      await tester.pumpAndSettle();
      expect(closed, isTrue);
      expect(handleHomeBack(), isFalse);
      expect(tester.takeException(), isNull);
    },
  );

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
                      onCancel: () {},
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

    await tester.ensureVisible(find.byType(TextField));
    await tester.enterText(find.byType(TextField), 'Landscape draft');
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Journey Date'));
    await tester.pumpAndSettle();
    expect(find.text('Journey Date').hitTestable(), findsOneWidget);
    await tester.ensureVisible(find.text('Save'));
    await tester.pumpAndSettle();
    expect(find.text('Save').hitTestable(), findsOneWidget);
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
          onCancel: () {},
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
          onCancel: () {},
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
                onCancel: () {},
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
  final List<JourneyInfo> savedMetadata = [];

  @override
  Future<void> crateApiApiUpdateJourneyMetadata({
    required String id,
    required JourneyInfo journeyInfo,
  }) async {
    savedMetadata.add(journeyInfo);
  }

  @override
  Future<JourneyHeader?> crateApiApiGetJourneyHeader({
    required String journeyId,
  }) async {
    final latest = savedMetadata.isEmpty ? null : savedMetadata.last;
    return JourneyHeader(
      id: journey.id,
      revision: latest == null ? journey.revision : 'updated',
      journeyDate: journey.journeyDate,
      createdAt: journey.createdAt,
      journeyType: journey.journeyType,
      journeyKind: latest?.journeyKind ?? journey.journeyKind,
      note: latest?.note ?? journey.note,
    );
  }

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

class _TestWakelockPlatform extends WakelockPlusMacOSPlugin {
  @override
  Future<void> toggle({required bool enable}) async {}

  @override
  Future<bool> get enabled async => false;
}
