import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_date_picker_dialog.dart';

void main() {
  testWidgets('landscape date picker keeps dates and confirmation reachable', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(640, 320);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('plugins.flutter.io/shared_preferences'),
      (call) async => call.method == 'getAll' ? <String, Object>{} : null,
    );
    const locale = Locale('en', 'US');
    const loader = AppTranslationLoader();
    final initialDate = DateTime(2024, 6, 15);
    DateTime? result;

    await tester.runAsync(() async {
      await EasyLocalization.ensureInitialized();
      await loader.load('assets/translations', locale);
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
              home: Builder(
                builder: (context) => Scaffold(
                  body: TextButton(
                    onPressed: () async {
                      result = await showAppDatePickerDialog(
                        context,
                        initialDate: initialDate,
                        firstDate: DateTime(2024),
                        lastDate: DateTime(2024, 12, 31),
                      );
                    },
                    child: const Text('Open'),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump(const Duration(seconds: 1));
    });
    await tester.pumpAndSettle();
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    // June 2024 has six rows. The last row must be reachable by scrolling
    // while the actions stay visible in the short landscape viewport.
    await tester.ensureVisible(find.text('30'));
    await tester.pumpAndSettle();
    expect(find.text('30').hitTestable(), findsOneWidget);
    expect(find.text('Cancel').hitTestable(), findsOneWidget);
    expect(find.text('OK').hitTestable(), findsOneWidget);
    await tester.tap(find.text('OK'));
    await tester.pumpAndSettle();
    expect(result, initialDate);
  });
}
