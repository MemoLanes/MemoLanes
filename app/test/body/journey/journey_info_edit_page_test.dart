import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/simple_date_utils.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const enUs = Locale('en', 'US');

  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
      const MethodChannel('plugins.flutter.io/shared_preferences'),
      (call) async => call.method == 'getAll' ? <String, Object>{} : null,
    );
    await EasyLocalization.ensureInitialized();
    await const AppTranslationLoader().load('assets/translations', enUs);
  });

  testWidgets('date picker supports the minimum journey date', (tester) async {
    final app = EasyLocalization(
      supportedLocales: const [enUs],
      path: 'assets/translations',
      assetLoader: const AppTranslationLoader(),
      fallbackLocale: enUs,
      child: Builder(
        builder: (context) => MaterialApp(
          localizationsDelegates: context.localizationDelegates,
          supportedLocales: context.supportedLocales,
          locale: context.locale,
          home: Scaffold(
            body: JourneyInfoEditPage(
              startTime: null,
              endTime: null,
              journeyDate: SimpleDate(1, 1, 1),
              note: null,
              saveData: (_) async {},
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

    await tester.tap(find.text('01-01-01'));
    await tester.pumpAndSettle();

    expect(find.byType(DatePickerDialog), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
