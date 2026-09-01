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

      final app = _buildTestApp(onRangeChanged: (_, _) {});

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
    },
  );

  testWidgets('mode menu remains usable on a 320px viewport', (tester) async {
    tester.view.physicalSize = const Size(320, 480);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.runAsync(() async {
      await tester.pumpWidget(_buildTestApp(onRangeChanged: (_, _) {}));
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

  testWidgets(
    'narrow custom dates wrap without changing the horizontal tap areas',
    (tester) async {
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
                child: SizedBox(
                  width: 110,
                  height: 60,
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
        ),
      );
      await tester.runAsync(() async {
        await tester.pumpWidget(app);
        await tester.pump(const Duration(seconds: 1));
      });
      await tester.pumpAndSettle();

      final renderedText = tester
          .widgetList<Text>(
            find.descendant(
              of: find.byType(TimeRangeOverlayPicker),
              matching: find.byType(Text),
            ),
          )
          .map((text) => text.data)
          .whereType<String>()
          .toList();
      expect(renderedText, contains('2020\n01-02'));
      expect(renderedText, contains('2024\n12-31'));
      expect(find.textContaining('…'), findsNothing);
      expect(tester.takeException(), isNull);

      final tapTargets = find.descendant(
        of: find.byType(TimeRangeOverlayPicker),
        matching: find.byType(InkWell),
      );
      expect(tapTargets, findsNWidgets(2));
      final fromRect = tester.getRect(tapTargets.at(0));
      final toRect = tester.getRect(tapTargets.at(1));
      expect(fromRect.top, toRect.top);
      expect(fromRect.bottom, toRect.bottom);
      expect(fromRect.right, lessThanOrEqualTo(toRect.left));
    },
  );

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
