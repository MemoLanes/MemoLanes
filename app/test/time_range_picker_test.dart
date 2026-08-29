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

  testWidgets(
      'mode menu keeps its content size and leaves the timeline interactive',
      (tester) async {
    tester.view.physicalSize = const Size(1080, 1920);
    tester.view.devicePixelRatio = 3;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final app = _buildTestApp(onRangeChanged: (_, __) {});

    await tester.runAsync(() async {
      await tester.pumpWidget(app);
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    await tester.tap(find.byType(TimeRangeControllerBall));
    await tester.pumpAndSettle();

    final periodText = find.text('Period');
    expect(periodText, findsOneWidget);
    final menuGlass = find.ancestor(
      of: periodText,
      matching: find.byType(BackdropFilter),
    );
    expect(menuGlass, findsOneWidget);
    final menuSize = tester.getSize(menuGlass);
    expect(menuSize.width, lessThan(tester.view.physicalSize.width / 3));
    expect(menuSize.height, lessThan(tester.view.physicalSize.height / 3));

    await tester.drag(find.byType(TimeRuler), const Offset(-80, 0));
    await tester.pumpAndSettle();
    expect(periodText, findsOneWidget);

    await tester.tapAt(const Offset(350, 20));
    await tester.pumpAndSettle();
    expect(periodText, findsNothing);
  });

  testWidgets('mode menu remains usable on a 320px viewport', (tester) async {
    tester.view.physicalSize = const Size(320, 480);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.runAsync(() async {
      await tester.pumpWidget(_buildTestApp(onRangeChanged: (_, __) {}));
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    await tester.tap(find.byType(TimeRangeControllerBall));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.text('Period'), findsOneWidget);
    expect(find.text('Granularity'), findsOneWidget);
    expect(find.text('Ground'), findsOneWidget);
  });

  testWidgets('defaults to the cumulative as-of range', (tester) async {
    final ranges = <(SimpleDate, SimpleDate)>[];
    final currentYear = DateTime.now().year;
    await tester.runAsync(() async {
      await tester.pumpWidget(
        _buildTestApp(
          onRangeChanged: (from, to) => ranges.add((from, to)),
        ),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();

    expect(ranges, isNotEmpty);
    expect(ranges.last.$1, SimpleDate(2020));
    expect(ranges.last.$2, SimpleDate(currentYear, 12, 31));
  });
}
