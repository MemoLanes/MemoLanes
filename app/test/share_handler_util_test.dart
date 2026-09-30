import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/share_handler_util.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:zikzak_share_handler/zikzak_share_handler.dart';

SharedMedia _media(String name) => SharedMedia(
  attachments: [
    SharedAttachment(path: '/shared/$name', type: SharedAttachmentType.file),
  ],
);

class _ShareHandler extends ShareHandlerPlatform {
  final events = StreamController<SharedMedia>.broadcast(sync: true);

  @override
  Future<SharedMedia?> getInitialSharedMedia() async => _media('cold.mldx');

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
    addTearDown(() async {
      await service.dispose();
      if (!uiReady.isCompleted) uiReady.complete();
      await handler.events.close();
    });

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
    expect(key.currentState, isNull);
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: const [Locale('en', 'US')],
        startLocale: const Locale('en', 'US'),
        path: 'unused',
        assetLoader: _Translations(),
        child: Builder(
          builder: (context) => MaterialApp(
            theme: AppTheme.light,
            navigatorKey: key,
            localizationsDelegates: context.localizationDelegates,
            supportedLocales: context.supportedLocales,
            locale: context.locale,
            home: const Scaffold(),
          ),
        ),
      ),
    );
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
}
