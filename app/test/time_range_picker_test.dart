import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/app_menu.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

import 'support/duo_hardware_test_data.dart';

const _loader = AppTranslationLoader();
const _enUs = Locale('en', 'US');

Widget _buildTestApp({
  required void Function(SimpleDate from, SimpleDate to) onRangeChanged,
  DuoBarEdge edge = DuoBarEdge.unknown,
}) {
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
        home: Scaffold(
          body: AppLayoutBuilder(
            builder: (_, _) => Stack(
              children: [
                MapOverlayPanel.controls(
                  child: TimeRangePicker(
                    earliestDate: SimpleDate(2020),
                    selectedJourneyKinds: const {JourneyKind.defaultKind},
                    onRangeChanged: onRangeChanged,
                  ),
                ),
              ],
            ),
          ),
        ),
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

  testWidgets('timeline starts cumulative and applies menu selections', (
    tester,
  ) async {
    final haptics = AppHaptics.debugHapticsEnabledOverride;
    AppHaptics.debugHapticsEnabledOverride = false;
    addTearDown(() => AppHaptics.debugHapticsEnabledOverride = haptics);
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
            onRangeChanged: (from, to) => ranges.add((from, to)),
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
}
