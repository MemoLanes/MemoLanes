import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/overlay/time_machine_overlay.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final backend = _TimelineApi();
  const locale = Locale('en', 'US');
  const loader = AppTranslationLoader();

  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
    await loader.load('assets/translations', locale);
    RustLib.initMock(api: backend);
  });
  tearDownAll(RustLib.dispose);

  testWidgets('timeline capture waits for the latest range and for scrolling', (
    tester,
  ) async {
    final haptics = AppHaptics.debugHapticsEnabledOverride;
    AppHaptics.debugHapticsEnabledOverride = false;
    addTearDown(() => AppHaptics.debugHapticsEnabledOverride = haptics);
    final key = GlobalKey<TimeMachineOverlayState>();
    final loaded = <api.MapRendererProxy?>[];
    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale],
          path: 'assets/translations',
          assetLoader: loader,
          startLocale: locale,
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
              home: Scaffold(
                body: AppLayoutBuilder(
                  builder: (_, _) => TimeMachineOverlay(
                    key: key,
                    onJourneyRangeLoaded: loaded.add,
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
    });
    await tester.pump();
    final state = key.currentState!;
    expect(state.canCaptureScreenshot, isFalse);
    expect(() => state.captureScreenshot(1), throwsStateError);
    final initial = _MapRenderer();
    // Both the picker and its initial ruler position can commit the range.
    for (final request in backend.requests) {
      request.complete(initial);
    }
    await tester.pumpAndSettle();
    expect(state.canCaptureScreenshot, isTrue);
    backend.requests.clear();

    final picker = tester.widget<TimeRangePicker>(find.byType(TimeRangePicker));
    picker.onRangeChanged(SimpleDate(2024), SimpleDate(2024, 12, 31));
    expect(state.canCaptureScreenshot, isFalse);
    // The displayed date already changed, although its debounced request has
    // not started. A screenshot must not combine it with the previous map.
    expect(backend.requests, isEmpty);
    await tester.pump(const Duration(milliseconds: 100));
    expect(backend.requests, hasLength(1));
    picker.onRangeCommitted!(SimpleDate(2025), SimpleDate(2025, 12, 31));
    backend.requests[0].complete(_MapRenderer());
    await tester.pump();
    expect(loaded, [initial]);
    expect(state.canCaptureScreenshot, isFalse);

    final latest = _MapRenderer();
    backend.requests[1].complete(latest);
    await tester.pumpAndSettle();
    expect(loaded, [initial, latest]);
    expect(state.canCaptureScreenshot, isTrue);

    final gesture = await tester.startGesture(
      tester.getCenter(find.byType(ListView)),
    );
    await gesture.moveBy(const Offset(80, 0));
    await tester.pump();
    expect(state.canCaptureScreenshot, isFalse);
    expect(() => state.captureScreenshot(1), throwsStateError);
    await gesture.cancel();
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _TimelineApi extends Fake implements RustLibApi {
  final requests = <Completer<api.MapRendererProxy>>[];

  @override
  api.LayerFilter crateApiApiGetCurrentMainMapLayerFilter() => api.LayerFilter(
    defaultKind: true,
    flightKind: false,
    currentJourney: true,
  );

  @override
  Future<DateTime?> crateApiApiEarliestJourneyDate() async => DateTime(2020);

  @override
  Future<api.MapRendererProxy>
  crateApiApiGetMapRendererProxyForJourneyDateRange({
    required DateTime fromDateInclusive,
    required DateTime toDateInclusive,
    required Set<JourneyKind> journeyKinds,
  }) {
    final request = Completer<api.MapRendererProxy>();
    requests.add(request);
    return request.future;
  }
}

class _MapRenderer extends Fake implements api.MapRendererProxy {}
