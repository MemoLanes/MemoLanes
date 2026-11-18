import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/onboarding/onboarding_page.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/share_handler_util.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:provider/provider.dart';
import 'package:zikzak_share_handler/zikzak_share_handler.dart';

import 'support/no_location_gps.dart';

const _homeKey = ValueKey('share-test-home');

SharedMedia _media(String name) => SharedMedia(
  attachments: [
    SharedAttachment(path: '/shared/$name', type: SharedAttachmentType.file),
  ],
);

class _ShareHandler extends ShareHandlerPlatform {
  final events = StreamController<SharedMedia>.broadcast(sync: true);
  final initialShare = Completer<SharedMedia?>();

  @override
  Future<SharedMedia?> getInitialSharedMedia() => initialShare.future;

  @override
  Future<void> resetInitialSharedMedia() async {}

  @override
  Stream<SharedMedia> get sharedMediaStream => events.stream;
}

class _Translations extends AssetLoader {
  @override
  Future<Map<String, dynamic>> load(String path, Locale locale) async => {
    'common': {'ok': 'OK', 'cancel': 'Cancel'},
    'import': {
      'shared_file': {
        'confirm_title': 'Import file',
        'confirm_message': 'Receive {}?',
      },
    },
  };
}

Widget _app(GlobalKey<NavigatorState> key) => EasyLocalization(
  supportedLocales: const [Locale('en', 'US')],
  startLocale: const Locale('en', 'US'),
  path: 'unused',
  assetLoader: _Translations(),
  child: Builder(
    builder: (context) => MaterialApp(
      theme: AppTheme.light,
      navigatorKey: key,
      localizationsDelegates: [
        ...GlobalMaterialLocalizations.delegates,
        ...context.localizationDelegates,
      ],
      supportedLocales: context.supportedLocales,
      locale: context.locale,
      home: const Scaffold(key: _homeKey),
    ),
  ),
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
  });

  for (final earlyShare in [false, true]) {
    test(
      'startup checks for ${earlyShare ? 'an early share event' : 'no incoming file'}',
      () async {
        final handler = _ShareHandler();
        final uiReady = Completer<void>();
        final service = ShareHandlerUtil(
          navigatorKey: GlobalKey<NavigatorState>(),
          uiReady: uiReady.future,
          handler: handler,
        )..init();
        addTearDown(() async {
          await service.dispose();
          uiReady.complete();
          await handler.events.close();
        });

        var showedStartupUi = false;
        final startup = service.runBeforeStartupShare((_) async {
          showedStartupUi = true;
        });
        if (earlyShare) handler.events.add(_media('early.mldx'));
        handler.initialShare.complete(null);
        await startup;
        expect(showedStartupUi, !earlyShare);
      },
    );
  }

  testWidgets('retains a cold share and updates the pending confirmation', (
    tester,
  ) async {
    final handler = _ShareHandler();
    final uiReady = Completer<void>();
    final key = GlobalKey<NavigatorState>();
    final service = ShareHandlerUtil(
      navigatorKey: key,
      uiReady: uiReady.future,
      handler: handler,
    );
    service.init();
    final startup = service.runBeforeStartupShare((_) async {
      fail('Startup UI must yield to a cold share');
    });
    handler.initialShare.complete(_media('cold.mldx'));
    addTearDown(() async {
      await service.dispose();
      if (!uiReady.isCompleted) uiReady.complete();
      await handler.events.close();
    });

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
    expect(key.currentState, isNull);
    await startup;
    await tester.pumpWidget(_app(key));
    await tester.pumpAndSettle();
    expect(find.byType(CommonDialog), findsNothing);

    uiReady.complete();
    await tester.pumpAndSettle();
    expect(find.text('Receive cold.mldx?'), findsOneWidget);
    handler.events.add(_media('latest.fwss'));
    await tester.pumpAndSettle();
    expect(find.text('Receive latest.fwss?'), findsOneWidget);
    expect(find.text('Receive cold.mldx?'), findsNothing);
    expect(find.byType(CommonDialog), findsOneWidget);

    // Invoke the action directly to avoid unrelated native haptics setup.
    tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .first
        .onPressed();
    await tester.pumpAndSettle();
    expect(find.byType(CommonDialog), findsNothing);
  });

  testWidgets('a late share interrupts the automatic tour and opens import', (
    tester,
  ) async {
    final handler = _ShareHandler();
    final uiReady = Completer<void>();
    final key = GlobalKey<NavigatorState>();
    final service = ShareHandlerUtil(
      navigatorKey: key,
      uiReady: uiReady.future,
      handler: handler,
    )..init();
    addTearDown(() async {
      await service.dispose();
      if (!uiReady.isCompleted) uiReady.complete();
      await handler.events.close();
    });

    await tester.pumpWidget(
      ChangeNotifierProvider<GpsManager>(
        create: (_) => NoLocationGps(),
        child: _app(key),
      ),
    );
    await tester.pumpAndSettle();
    final home = tester.element(find.byKey(_homeKey));
    const tour = OnboardingPage(map: SizedBox.expand());
    final tourPage = find.byType(OnboardingPage);
    bool? completed;
    final startup = service.runBeforeStartupShare((shareArrivals) async {
      completed = await tour.show(home, interruptOn: shareArrivals);
    });
    unawaited(
      startup.then((_) {
        if (!uiReady.isCompleted) uiReady.complete();
      }),
    );
    handler.initialShare.complete(null);
    await tester.pumpAndSettle();
    expect(tourPage, findsOneWidget);

    handler.events.add(SharedMedia(content: 'text without a file'));
    await tester.pump();
    expect(tourPage, findsOneWidget);

    handler.events.add(_media('late.mldx'));
    await tester.pumpAndSettle();
    expect(tourPage, findsNothing);
    expect(completed, isFalse);
    expect(find.text('Receive late.mldx?'), findsOneWidget);

    tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .first
        .onPressed();
    await tester.pumpAndSettle();
    final replay = tour.show(home);
    await tester.pumpAndSettle();
    handler.events.add(_media('latest.mldx'));
    await tester.pumpAndSettle();
    expect(tourPage, findsOneWidget);
    tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .first
        .onPressed();
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('onboarding-skip')));
    await tester.pumpAndSettle();
    expect(await replay, isTrue);
  });
}
