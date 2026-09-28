import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/body/settings/mldx_import_page.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/component/import_loading_page.dart';
import 'package:memolanes/common/component/multi_journey_import_page.dart';
import 'package:memolanes/common/share_handler_util.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:zikzak_share_handler/zikzak_share_handler.dart';

SharedMedia _media(String name) => SharedMedia(
  attachments: [
    SharedAttachment(path: '/shared/$name', type: SharedAttachmentType.file),
  ],
);

class _ShareHandler extends ShareHandlerPlatform {
  _ShareHandler({Future<SharedMedia?>? initial})
    : initial = initial ?? Future.value(_media('cold.mldx'));

  final Future<SharedMedia?> initial;
  final events = StreamController<SharedMedia>.broadcast(sync: true);
  int initialReads = 0;
  int initialResets = 0;

  @override
  Future<SharedMedia?> getInitialSharedMedia() {
    initialReads++;
    return initial;
  }

  @override
  Future<void> resetInitialSharedMedia() async {
    initialResets++;
  }

  @override
  Stream<SharedMedia> get sharedMediaStream => events.stream;
}

class _ImportApi extends Fake implements RustLibApi {
  late _MldxReader reader;

  @override
  Future<OpaqueMldxReader> crateApiImportOpaqueMldxReaderOpen({
    required String mldxFilePath,
  }) async => reader;
}

class _MldxReader extends Fake implements OpaqueMldxReader {
  final analysis =
      Completer<List<(JourneyHeader, MldxJourneyImportAnalyzeResult)>>();

  @override
  Future<List<(JourneyHeader, MldxJourneyImportAnalyzeResult)>> analyze() =>
      analysis.future;

  @override
  Future<void> importJourneys({Set<String>? journeyIds}) async {}
}

final _journey = JourneyHeader(
  id: 'journey',
  revision: '1',
  journeyDate: DateTime(2024, 1, 1),
  createdAt: DateTime(2024, 1, 1),
  journeyType: JourneyType.vector,
  journeyKind: JourneyKind.defaultKind,
);

class _Translations extends AssetLoader {
  @override
  Future<Map<String, dynamic>> load(String path, Locale locale) async => {
    'common': {'ok': 'OK', 'cancel': 'Cancel', 'info': 'Info'},
    'import': {
      'shared_file': {
        'confirm_title': 'Import file',
        'confirm_message': 'Receive {}?',
        'multi_message': 'Share one file at a time.',
        'busy_title': 'Import in progress',
        'busy_message': 'Complete or cancel this import before sharing again.',
      },
      'unsupported_file_failed': 'Unsupported file',
      'parsing_failed': 'Parsing failed',
      'successful': 'Import successful',
      'mldx_preview': {
        'title': 'Import archive',
        'all_skipped': 'Already imported {} journeys.',
        'last_modified': 'Modified {}',
        'new_count': '{} new',
        'summary_counts': '{} new, {} conflicts, {} skipped',
      },
      'journey_selection': {
        'list_section_title': 'Journeys',
        'select_all': 'Select all',
        'deselect_all': 'Deselect all',
        'confirm_import': 'Import',
      },
      'loading': {
        'title': 'Loading file',
        'description': 'Preparing preview',
        'keep_open_hint': 'Keep the app open',
      },
    },
    'data': {
      'import_data': {'title': 'Import data'},
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
      localizationsDelegates: context.localizationDelegates,
      supportedLocales: context.supportedLocales,
      locale: context.locale,
      home: const Scaffold(body: Text('Ready')),
    ),
  ),
);

final _loadingPages = find.byWidgetPredicate(
  (widget) => widget is ImportLoadingPage,
  skipOffstage: false,
);

// Exercise the dialog actions without initializing unrelated native haptics.
void _pressDialogButton(WidgetTester tester, String text, {String? content}) {
  final dialogs = tester.widgetList<CommonDialog>(find.byType(CommonDialog));
  final dialog = content == null
      ? dialogs.last
      : dialogs.singleWhere((dialog) => dialog.content == content);
  dialog.buttons.singleWhere((button) => button.text == text).onPressed();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final importApi = _ImportApi();

  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
    RustLib.initMock(api: importApi);
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMessageHandler(
          'dev.flutter.pigeon.wakelock_plus_platform_interface.WakelockPlusApi.toggle',
          (_) async => const StandardMessageCodec().encodeMessage([null]),
        );
  });

  late _ShareHandler handler;
  late Completer<void> uiReady;
  late GlobalKey<NavigatorState> key;
  late ShareHandlerUtil service;

  void start({Future<SharedMedia?>? initial}) {
    handler = _ShareHandler(initial: initial);
    uiReady = Completer<void>();
    key = GlobalKey<NavigatorState>();
    service = ShareHandlerUtil(
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
  }

  Future<_MldxReader> startLoadingMldx(WidgetTester tester) async {
    final reader = _MldxReader();
    importApi.reader = reader;
    start();
    await tester.pumpWidget(_app(key));
    await tester.pumpAndSettle();
    uiReady.complete();
    await tester.pumpAndSettle();
    _pressDialogButton(tester, 'OK');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(_loadingPages, findsOneWidget);
    handler.events.add(_media('blocked.fwss'));
    await tester.pump();
    expect(find.text('Import in progress'), findsOneWidget);
    handler.events.add(_media('also-blocked.fwss'));
    await tester.pump();
    expect(find.text('Import in progress'), findsOneWidget);
    return reader;
  }

  test('resolves the final extension, ignoring case', () {
    expect(
      ShareHandlerUtil.resolveImportType('/data/轨迹.GPX'),
      ImportType.vector,
    );
    expect(
      ShareHandlerUtil.resolveImportType('/data/folder.gpx/backup.FWSS'),
      ImportType.fow,
    );
    expect(ShareHandlerUtil.resolveImportType('/data/track.gpx.bak'), isNull);
    expect(ShareHandlerUtil.resolveImportType('/data/file'), isNull);
  });

  testWidgets('ignores blank paths and preserves spaces in file names', (
    tester,
  ) async {
    start(
      initial: Future.value(
        SharedMedia(
          attachments: [
            for (final path in ['', ' \t '])
              SharedAttachment(path: path, type: SharedAttachmentType.file),
          ],
        ),
      ),
    );
    await tester.pumpWidget(_app(key));
    await tester.pumpAndSettle();
    uiReady.complete();
    await tester.pumpAndSettle();
    expect(find.byType(CommonDialog), findsNothing);

    handler.events.add(_media(' spaced .MLDX'));
    await tester.pumpAndSettle();
    expect(find.text('Receive  spaced .MLDX?'), findsOneWidget);
    key.currentState!.pop();
    await tester.pumpAndSettle();
  });

  testWidgets(
    'retains cold share until UI startup completes; receives idle hot share',
    (tester) async {
      start();
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
      expect(key.currentState, isNull);

      await tester.pumpWidget(_app(key));
      await tester.pumpAndSettle();
      expect(find.byType(CommonDialog), findsNothing);

      uiReady.complete();
      await tester.pumpAndSettle();
      expect(find.text('Receive cold.mldx?'), findsOneWidget);
      expect(handler.initialResets, 1);
      key.currentState!.pop();
      await tester.pumpAndSettle();

      handler.events.add(_media('hot.fwss'));
      await tester.pumpAndSettle();
      expect(find.text('Receive hot.fwss?'), findsOneWidget);
      key.currentState!.pop();
      await tester.pumpAndSettle();
      expect(find.byType(CommonDialog), findsNothing);
    },
  );

  testWidgets(
    'keeps the latest share before confirmation and ignores a stale initial share',
    (tester) async {
      final initial = Completer<SharedMedia?>();
      start(initial: initial.future);
      service.init();
      // Keep the latest stream event, even before UI startup. An initial share
      // returned later must not replace it with older data.
      handler.events.add(_media('cold.mldx'));
      handler.events.add(_media('next.fwss'));
      await tester.pump();
      initial.complete(_media('cold.mldx'));
      await tester.pump();
      expect(handler.initialReads, 1);

      await tester.pumpWidget(_app(key));
      await tester.pumpAndSettle();
      uiReady.complete();
      await tester.pumpAndSettle();
      expect(find.text('Receive next.fwss?'), findsOneWidget);
      expect(find.byType(CommonDialog), findsOneWidget);
      handler.events.add(_media('during-confirm.fwss'));
      await tester.pumpAndSettle();
      expect(find.text('Receive during-confirm.fwss?'), findsOneWidget);
      expect(find.text('Receive next.fwss?'), findsNothing);
      expect(find.byType(CommonDialog), findsOneWidget);
      _pressDialogButton(tester, 'Cancel');
      await tester.pumpAndSettle();
      expect(find.byType(CommonDialog), findsNothing);

      handler.events.add(_media('cold.mldx'));
      await tester.pumpAndSettle();
      expect(find.text('Receive cold.mldx?'), findsOneWidget);
      key.currentState!.pop();
      await tester.pumpAndSettle();
    },
  );

  testWidgets('updates a multi-file notice when a single file is shared', (
    tester,
  ) async {
    start();
    await tester.pumpWidget(_app(key));
    await tester.pumpAndSettle();
    uiReady.complete();
    await tester.pumpAndSettle();
    handler.events.add(
      SharedMedia(
        attachments: [
          ..._media('first.fwss').attachments!,
          ..._media('second.fwss').attachments!,
        ],
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Share one file at a time.'), findsOneWidget);
    expect(find.text('Cancel'), findsNothing);
    expect(find.byType(CommonDialog), findsOneWidget);

    handler.events.add(_media('latest.fwss'));
    await tester.pumpAndSettle();
    expect(find.text('Receive latest.fwss?'), findsOneWidget);
    expect(find.byType(CommonDialog), findsOneWidget);
    _pressDialogButton(tester, 'Cancel');
    await tester.pumpAndSettle();
  });

  testWidgets(
    'locks the latest file on confirmation and shows one busy notice',
    (tester) async {
      start();
      await tester.pumpWidget(_app(key));
      await tester.pumpAndSettle();
      uiReady.complete();
      await tester.pumpAndSettle();

      // Use an unsupported file to exercise the real import dispatch without
      // requiring a Rust runtime. Its error dialog keeps the flow active.
      expect(find.text('Receive cold.mldx?'), findsOneWidget);
      handler.events.add(_media('latest.unknown'));
      // Confirm before the next frame: the action must use the latest share,
      // rather than the file captured by the previous dialog build.
      _pressDialogButton(tester, 'OK');
      handler.events.add(_media('blocked.fwss'));
      await tester.pumpAndSettle();
      expect(find.text('Import in progress'), findsOneWidget);
      expect(find.text('Receive blocked.fwss?'), findsNothing);

      handler.events.add(_media('another.fwss'));
      await tester.pumpAndSettle();
      expect(find.byType(CommonDialog), findsNWidgets(2));
      _pressDialogButton(tester, 'OK');
      await tester.pumpAndSettle();
      expect(find.text('Unsupported file'), findsOneWidget);

      // Closing the notice does not unlock the import itself.
      handler.events.add(_media('still-blocked.fwss'));
      await tester.pumpAndSettle();
      expect(find.text('Import in progress'), findsOneWidget);
      _pressDialogButton(tester, 'OK');
      await tester.pumpAndSettle();
      _pressDialogButton(tester, 'OK');
      await tester.pumpAndSettle();
      expect(find.byType(CommonDialog), findsNothing);

      handler.events.add(_media('latest.unknown'));
      await tester.pumpAndSettle();
      expect(find.text('Receive latest.unknown?'), findsOneWidget);
      _pressDialogButton(tester, 'Cancel');
      await tester.pumpAndSettle();
    },
  );

  test('disposing cancels subscription and pending delivery', () async {
    start();
    await Future<void>.delayed(Duration.zero);
    expect(handler.events.hasListener, isTrue);
    await service.dispose();
    expect(handler.events.hasListener, isFalse);
    uiReady.complete();
    handler.events.add(_media('after-dispose.mldx'));
    await Future<void>.delayed(Duration.zero);
    // There is no Navigator: attempting delivery after disposal would fail.
    expect(key.currentState, isNull);
  });

  for (final completeNormally in [false, true]) {
    testWidgets(
      'busy notice does not replace loading route; '
      '${completeNormally ? 'successful import' : 'cancellation'} unlocks sharing',
      (tester) async {
        final reader = await startLoadingMldx(tester);
        reader.analysis.complete([
          (_journey, MldxJourneyImportAnalyzeResult.new_),
        ]);
        await tester.pumpAndSettle();
        expect(find.byType(MldxImportPage), findsOneWidget);
        expect(_loadingPages, findsNothing);
        expect(find.text('Import in progress'), findsOneWidget);

        if (completeNormally) {
          final confirmation = tester
              .widget<MultiJourneyImportPage>(
                find.byType(MultiJourneyImportPage),
              )
              .onConfirm();
          await tester.pumpAndSettle();
          // Invoke the success action while the independent notice is still
          // present: it must not prevent the import page from closing.
          _pressDialogButton(tester, 'OK', content: 'Import successful');
          await tester.pumpAndSettle();
          await confirmation;
        } else {
          // The navigator must pop the preview, rather than the busy notice.
          key.currentState!.pop(false);
          await tester.pumpAndSettle();
        }

        expect(find.byType(MldxImportPage), findsNothing);
        expect(find.byType(CommonDialog), findsNothing);
        expect(key.currentState!.canPop(), isFalse);
        handler.events.add(_media('retry.fwss'));
        await tester.pumpAndSettle();
        expect(find.text('Receive retry.fwss?'), findsOneWidget);
        _pressDialogButton(tester, 'Cancel');
        await tester.pumpAndSettle();
      },
    );
  }

  for (final fails in [false, true]) {
    testWidgets('busy notice does not block loading route cleanup after '
        '${fails ? 'a parsing error' : 'all journeys are skipped'}', (
      tester,
    ) async {
      final reader = await startLoadingMldx(tester);
      if (fails) {
        reader.analysis.completeError(StateError('Invalid file'));
      } else {
        reader.analysis.complete([
          (_journey, MldxJourneyImportAnalyzeResult.unchanged),
        ]);
      }
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text('Import in progress'), findsOneWidget);
      _pressDialogButton(
        tester,
        'OK',
        content: fails ? 'Parsing failed' : 'Already imported 1 journeys.',
      );
      await tester.pumpAndSettle();
      expect(_loadingPages, findsNothing);
      expect(find.byType(CommonDialog), findsNothing);
      expect(key.currentState!.canPop(), isFalse);
      handler.events.add(_media('retry.fwss'));
      await tester.pumpAndSettle();
      expect(find.text('Receive retry.fwss?'), findsOneWidget);
      _pressDialogButton(tester, 'Cancel');
      await tester.pumpAndSettle();
    });
  }

  testWidgets('disposing removes a visible busy notice', (tester) async {
    final reader = await startLoadingMldx(tester);
    reader.analysis.complete([(_journey, MldxJourneyImportAnalyzeResult.new_)]);
    await tester.pumpAndSettle();
    expect(find.text('Import in progress'), findsOneWidget);
    await tester.runAsync(service.dispose);
    await tester.pump();
    expect(find.text('Import in progress'), findsNothing);
    expect(handler.events.hasListener, isFalse);
    key.currentState!.pop(false);
    await tester.pumpAndSettle();
    expect(find.byType(CommonDialog), findsNothing);
  });
}
