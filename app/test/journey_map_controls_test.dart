import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/map/overlay/journey_detail_card.dart';
import 'package:memolanes/body/map/overlay/journey_more_dialog.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/journey_header.dart';

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

  final journey = JourneyHeader(
    id: 'original',
    revision: 'revision',
    journeyDate: DateTime.utc(2023, 11, 14),
    createdAt: DateTime.utc(2023, 11, 14),
    journeyType: JourneyType.vector,
    journeyKind: JourneyKind.defaultKind,
    note: 'Original note',
  );

  testWidgets('saving edits submits metadata and waits for completion', (
    tester,
  ) async {
    final saved = <JourneyInfo>[];
    final saveFinished = Completer<void>();
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailCard(
          journey: journey,
          isEditing: true,
          onExport: () {},
          onEdit: () {},
          onMore: () {},
          onSave: (info) async {
            saved.add(info);
            await saveFinished.future;
          },
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Changed note');
    await tester.tap(find.text('Save'));
    await tester.pump();
    expect(tester.widget<TextField>(find.byType(TextField)).readOnly, isTrue);
    await tester.tap(find.text('Save'));
    expect(saved, hasLength(1));
    saveFinished.complete();
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.note, 'Changed note');
    expect(saved.single.journeyDate, journey.journeyDate);
    expect(saved.single.journeyKind, journey.journeyKind);
    expect(tester.widget<TextField>(find.byType(TextField)).readOnly, isFalse);
  });

  testWidgets('failed save keeps the draft and allows retry', (tester) async {
    final saved = <JourneyInfo>[];
    await pumpApp(
      tester,
      Scaffold(
        body: JourneyDetailCard(
          journey: journey,
          isEditing: true,
          onExport: () {},
          onEdit: () {},
          onMore: () {},
          onSave: (info) async {
            saved.add(info);
            if (saved.length == 1) throw StateError('Storage unavailable');
          },
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Keep my draft');
    await tester.tap(find.text('Save'));
    // The save button remains busy until the error dialog is dismissed.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(
      find.text('Something went wrong. Please try again.'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
    invokeConfirmationAction(tester, 'OK');
    await tester.pumpAndSettle();
    expect(find.text('Keep my draft'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).readOnly, isFalse);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved, hasLength(2));
    expect(saved[1].note, saved[0].note);
    expect(find.byType(CommonDialog), findsNothing);
  });

  testWidgets('more ignores duplicate actions during navigation', (
    tester,
  ) async {
    final results = <JourneyMoreAction?>[];
    await pumpApp(
      tester,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () async =>
                results.add(await showJourneyMoreDialog(context)),
            child: const Text('Open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    final delete = tester
        .widget<AppButton>(
          find.ancestor(
            of: find.text('Delete Journey'),
            matching: find.byType(AppButton),
          ),
        )
        .onPressed!;
    // Invoke stale callbacks before a frame is drawn: a confirmation must not
    // stack twice.
    delete();
    delete();
    await tester.pumpAndSettle();
    expect(find.byType(CommonDialog), findsOneWidget);
    expect(results, isEmpty);
    final cancel = tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .singleWhere((button) => button.text == 'Cancel')
        .onPressed;
    cancel();
    cancel();
    await tester.pumpAndSettle();
    expect(find.text('Delete Journey').hitTestable(), findsOneWidget);
    expect(results, isEmpty);
    delete();
    await tester.pumpAndSettle();
    final confirm = tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .singleWhere((button) => button.text == 'Delete')
        .onPressed;
    confirm();
    confirm();
    await tester.pumpAndSettle();
    expect(results, [JourneyMoreAction.delete]);
    expect(find.text('Open').hitTestable(), findsOneWidget);
    expect(find.byType(Dialog), findsNothing);
  });

  testWidgets('cancel deletion returns to more and allows retrying', (
    tester,
  ) async {
    final results = <JourneyMoreAction?>[];
    await pumpApp(
      tester,
      Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () async =>
                results.add(await showJourneyMoreDialog(context)),
            child: const Text('Open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    // Both Cancel and system Back must leave the menu open without a result.
    for (final cancelWithBack in [false, true]) {
      await tester.tap(find.text('Delete Journey'));
      await tester.pumpAndSettle();
      expect(find.text('Delete this journey record?'), findsOneWidget);
      if (cancelWithBack) {
        await tester.binding.handlePopRoute();
      } else {
        invokeConfirmationAction(tester, 'Cancel');
      }
      await tester.pumpAndSettle();
      expect(find.text('Delete this journey record?'), findsNothing);
      expect(find.text('Delete Journey').hitTestable(), findsOneWidget);
      expect(results, isEmpty);
    }
    await tester.tap(find.text('Delete Journey'));
    await tester.pumpAndSettle();
    invokeConfirmationAction(tester, 'Delete');
    await tester.pumpAndSettle();
    expect(results, [JourneyMoreAction.delete]);
    expect(find.byType(Dialog), findsNothing);
  });
}

void invokeConfirmationAction(WidgetTester tester, String label) {
  expect(find.text(label).hitTestable(), findsOneWidget);
  // Exercise the real dialog navigation callback without requiring the
  // device-only MMKV/haptic setup used by CommonDialog's tap wrapper.
  tester
      .widget<CommonDialog>(find.byType(CommonDialog))
      .buttons
      .singleWhere((button) => button.text == label)
      .onPressed();
}
