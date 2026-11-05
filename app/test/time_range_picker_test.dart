import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/map/overlay/time_machine_overlay.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/body/time_machine/time_ruler.dart'
    show kRulerSnapDelay, kRulerUnitSpacing;
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/app_menu.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

import 'support/duo_hardware_test_data.dart';

const _loader = AppTranslationLoader();
const _enUs = Locale('en', 'US');

Widget _buildTestApp(Widget body, {DuoBarEdge edge = DuoBarEdge.unknown}) {
  return EasyLocalization(
    supportedLocales: const [_enUs],
    path: 'assets/translations',
    assetLoader: _loader,
    fallbackLocale: _enUs,
    child: Builder(
      builder: (context) => MaterialApp(
        theme: AppTheme.light,
        localizationsDelegates: [
          ...GlobalMaterialLocalizations.delegates,
          ...context.localizationDelegates,
        ],
        supportedLocales: context.supportedLocales,
        locale: context.locale,
        builder: (_, child) => DuoHardwareScope(
          hardware: edge == DuoBarEdge.unknown
              ? DuoHardware.none
              : const DuoHardware(
                  hasHinge: true,
                  status: DuoHingeStatus.fullyOpen,
                ),
          child: DuoBarScope(edge: edge, child: child!),
        ),
        home: Scaffold(body: AppLayoutBuilder(builder: (_, _) => body)),
      ),
    ),
  );
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
    await _loader.load('assets/translations', _enUs);
  });

  setUp(() {
    final haptics = AppHaptics.debugHapticsEnabledOverride;
    AppHaptics.debugHapticsEnabledOverride = false;
    addTearDown(() => AppHaptics.debugHapticsEnabledOverride = haptics);
  });

  testWidgets('timeline starts cumulative and applies menu selections', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final today = SimpleDate.today();
    final earliest = SimpleDate(2020);
    final yearEnd = SimpleDate(today.year, 12, 31);
    for (final (size, edge) in [
      (const Size(320, 720), DuoBarEdge.unknown),
      (const Size(951, 669), DuoBarEdge.right),
    ]) {
      tester.view.physicalSize = size;
      final ranges = <(SimpleDate, SimpleDate)>[];
      await tester.runAsync(() async {
        await tester.pumpWidget(
          _buildTestApp(
            Stack(
              children: [
                MapOverlayPanel.controls(
                  child: TimeRangePicker(
                    earliestDate: earliest,
                    selectedJourneyKinds: const {JourneyKind.defaultKind},
                    onRangeChanged: (from, to) => ranges.add((from, to)),
                  ),
                ),
              ],
            ),
            edge: edge,
          ),
        );
        await tester.pump(const Duration(seconds: 1));
      });
      await tester.pumpAndSettle();
      expect(ranges.last, (earliest, yearEnd));
      await tester.tap(find.byType(AppMenu));
      await tester.pumpAndSettle();
      for (final (key, expected) in [
        ('time_machine.menu_day', (earliest, today)),
        ('time_machine.menu_year', (earliest, yearEnd)),
        ('time_machine.menu_view_period', (SimpleDate(today.year), yearEnd)),
      ]) {
        final option = find.text(tr(key)).last;
        await tester.ensureVisible(option);
        await tester.pumpAndSettle();
        await tester.tap(option);
        await tester.pumpAndSettle();
        expect(ranges.last, expected);
      }
      await tester.tap(find.byType(AppMenu));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    }
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets('timeline screenshots wait for the settled and loaded range', (
    tester,
  ) async {
    final backend = _TimelineApi();
    RustLib.initMock(api: backend);
    addTearDown(RustLib.dispose);
    final key = GlobalKey<TimeMachineOverlayState>();
    var appliedRanges = 0;
    await tester.runAsync(() async {
      await tester.pumpWidget(
        _buildTestApp(
          TimeMachineOverlay(
            key: key,
            onJourneyRangeLoaded: (_) => appliedRanges++,
          ),
        ),
      );
      await tester.pump();
    });
    await tester.pump();
    final state = key.currentState!;

    Future<void> completeLoads() async {
      for (final request in backend.requests) {
        request.complete(_MapRenderer());
      }
      backend.requests.clear();
      await tester.pumpAndSettle();
    }

    await completeLoads();
    expect(state.screenshotDateRange, isNotNull);

    final picker = tester.widget<TimeRangePicker>(find.byType(TimeRangePicker));
    picker.onRangeChanged(SimpleDate(2024), SimpleDate(2024, 12, 31));
    await tester.pump(const Duration(milliseconds: 100));
    final obsolete = backend.requests.removeLast();
    picker.onRangeChanged(SimpleDate(2025), SimpleDate(2025, 12, 31));
    obsolete.complete(_MapRenderer());
    await tester.pump();
    expect(appliedRanges, 1);
    expect(state.screenshotDateRange, isNull);

    await tester.pump(const Duration(milliseconds: 100));
    await completeLoads();
    expect(
      state.screenshotDateRange,
      DateTimeRange(start: DateTime(2025), end: DateTime(2025, 12, 31)),
    );

    final ruler = tester.widget<ListView>(find.byType(ListView)).controller!;
    ruler.jumpTo(ruler.offset - 1.25 * kRulerUnitSpacing);
    expect(state.screenshotDateRange, isNull, reason: 'Before the snap');
    await tester.pump(kRulerSnapDelay);
    expect(state.screenshotDateRange, isNull, reason: 'The ruler is snapping');
    await tester.pump(const Duration(milliseconds: 250));
    expect(state.screenshotDateRange, isNull, reason: 'Range still loading');
    await completeLoads();
    final snappedRange = DateTimeRange(
      start: DateTime(2020),
      end: DateTime(DateTime.now().year - 1, 12, 31),
    );
    expect(state.screenshotDateRange, snappedRange);

    ruler.jumpTo(ruler.offset - 0.25 * kRulerUnitSpacing);
    expect(state.screenshotDateRange, isNull);
    await tester.pumpAndSettle();
    expect(state.screenshotDateRange, snappedRange);
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
