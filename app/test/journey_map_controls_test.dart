import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/map/overlay/journey_time_picker_dialog.dart';
import 'package:memolanes/common/app_translation_loader.dart';

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

  for (final use24HourFormat in [false, true]) {
    for (final size in [const Size(320, 480), const Size(560, 320)]) {
      testWidgets(
        'time dialog preserves time and cancels at $size, 24h=$use24HourFormat',
        (tester) async {
          tester.view.physicalSize = size;
          tester.view.devicePixelRatio = 1;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          final results = <TimeOfDay?>[];
          await pumpApp(
            tester,
            Builder(
              builder: (context) => Scaffold(
                body: TextButton(
                  onPressed: () async {
                    results.add(
                      await showDialog<TimeOfDay>(
                        context: context,
                        builder: (context) => MediaQuery(
                          data: MediaQuery.of(context)
                              .copyWith(alwaysUse24HourFormat: use24HourFormat),
                          child: const CompactJourneyTimeDialog(
                            initialTime: TimeOfDay(hour: 23, minute: 7),
                          ),
                        ),
                      ),
                    );
                  },
                  child: const Text('Open'),
                ),
              ),
            ),
          );
          for (final action in ['OK', 'Cancel']) {
            await tester.tap(find.text('Open'));
            await tester.pumpAndSettle();
            expect(tester.takeException(), isNull);
            await tester.ensureVisible(find.text(action));
            await tester.tap(find.text(action));
            await tester.pumpAndSettle();
            expect(find.text('Open').hitTestable(), findsOneWidget);
          }
          expect(results, [const TimeOfDay(hour: 23, minute: 7), null]);
        },
      );
    }
  }
}
