import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/component/app_menu.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

const _loader = AppTranslationLoader();
const _enUs = Locale('en', 'US');

Widget _buildTestApp({
  required void Function(SimpleDate from, SimpleDate to) onRangeChanged,
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
        home: Scaffold(
          body: Align(
            alignment: Alignment.bottomCenter,
            child: Padding(
              padding: const EdgeInsets.only(left: 20, right: 20, bottom: 80),
              child: TimeRangePicker(
                earliestDate: SimpleDate(2020),
                selectedJourneyKinds: const {JourneyKind.defaultKind},
                onRangeChanged: onRangeChanged,
              ),
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

  testWidgets('defaults to the cumulative as-of range', (tester) async {
    final ranges = <(SimpleDate, SimpleDate)>[];
    final currentYear = DateTime.now().year;
    await tester.runAsync(() async {
      await tester.pumpWidget(
        _buildTestApp(onRangeChanged: (from, to) => ranges.add((from, to))),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    expect(ranges, isNotEmpty);
    expect(ranges.last.$1, SimpleDate(2020));
    expect(ranges.last.$2, SimpleDate(currentYear, 12, 31));
  });

  testWidgets('timeline menu keeps full labels reachable in narrow viewports', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    await tester.runAsync(() async {
      await tester.pumpWidget(_buildTestApp(onRangeChanged: (_, _) {}));
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();
    for (final size in [
      const Size(800, 600),
      const Size(393, 852),
      const Size(320, 720),
    ]) {
      tester.view.physicalSize = size;
      await tester.pumpAndSettle();
      await tester.tap(find.byType(AppMenu));
      await tester.pumpAndSettle();
      for (final key in [
        'time_machine.menu_title_view',
        'time_machine.menu_view_period',
        'time_machine.menu_view_as_of',
        'time_machine.menu_view_custom',
        'time_machine.menu_title_granularity',
        'time_machine.menu_year',
        'time_machine.menu_month',
        'time_machine.menu_day',
        'time_machine.menu_title_layer',
        'journey_kind.default',
        'journey_kind.flight',
      ]) {
        final label = find.text(tr(key)).last;
        await tester.ensureVisible(label);
        await tester.pumpAndSettle();
        expect(label.hitTestable(), findsOneWidget);
        expect(
          tester.renderObject<RenderParagraph>(label).didExceedMaxLines,
          isFalse,
          reason: 'The full ${tr(key)} label must be readable.',
        );
      }
      expect(tester.takeException(), isNull);
    }
  });
}
