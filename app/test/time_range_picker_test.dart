import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

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
        localizationsDelegates: context.localizationDelegates,
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

  testWidgets('custom date tiles use the shared app date picker', (
    tester,
  ) async {
    final app = EasyLocalization(
      supportedLocales: const [_enUs],
      path: 'assets/translations',
      assetLoader: _loader,
      fallbackLocale: _enUs,
      child: Builder(
        builder: (context) => MaterialApp(
          localizationsDelegates: context.localizationDelegates,
          supportedLocales: context.supportedLocales,
          locale: context.locale,
          home: Scaffold(
            body: Center(
              child: TimeRangeOverlayPicker(
                fromDate: DateTime(2020, 1, 2),
                toDate: DateTime(2024, 12, 31),
                earliest: DateTime(2020),
                onFromChanged: (_) {},
                onToChanged: (_) {},
              ),
            ),
          ),
        ),
      ),
    );

    await tester.runAsync(() async {
      await tester.pumpWidget(app);
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    final dateTiles = find.descendant(
      of: find.byType(TimeRangeOverlayPicker),
      matching: find.byType(InkWell),
    );
    await tester.tap(dateTiles.first);
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byType(Dialog), findsOneWidget);
    expect(find.byType(DatePickerDialog), findsNothing);
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
}
