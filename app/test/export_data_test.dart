import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/body/settings/settings_category_pages.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/export_data.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/legacy_raw_data.dart';
import 'package:provider/provider.dart';
import 'package:wakelock_plus/wakelock_plus.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  testWidgets(
    'export dialog routes formats and blocks interaction while generating',
    (tester) async {
      final originalWakelock = wakelockPlusPlatformInstance;
      final originalHaptics = AppHaptics.debugHapticsEnabledOverride;
      addTearDown(() {
        wakelockPlusPlatformInstance = originalWakelock;
        AppHaptics.debugHapticsEnabledOverride = originalHaptics;
      });
      wakelockPlusPlatformInstance = _TestWakelockPlatform();
      AppHaptics.debugHapticsEnabledOverride = false;
      final backend = _ExportApi();
      RustLib.initMock(api: backend);
      addTearDown(RustLib.dispose);
      final gpsManager = _TestGpsManager();
      addTearDown(gpsManager.dispose);
      final navigatorKey = GlobalKey<NavigatorState>();

      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      const preferences = MethodChannel(
        'plugins.flutter.io/shared_preferences',
      );
      const paths = MethodChannel('plugins.flutter.io/path_provider');
      addTearDown(() {
        messenger.setMockMethodCallHandler(preferences, null);
        messenger.setMockMethodCallHandler(paths, null);
      });
      messenger.setMockMethodCallHandler(
        preferences,
        (call) async => call.method == 'getAll' ? <String, Object>{} : null,
      );
      messenger.setMockMethodCallHandler(paths, (_) async => '/tmp');
      await EasyLocalization.ensureInitialized();
      await tester.runAsync(() async {
        await const AppTranslationLoader().load(
          'assets/translations',
          const Locale('en', 'US'),
        );
        await tester.pumpWidget(
          EasyLocalization(
            supportedLocales: const [Locale('en', 'US')],
            path: 'assets/translations',
            assetLoader: const AppTranslationLoader(),
            fallbackLocale: const Locale('en', 'US'),
            child: Builder(
              builder: (context) => MaterialApp(
                navigatorKey: navigatorKey,
                theme: AppTheme.light,
                localizationsDelegates: [
                  ...GlobalMaterialLocalizations.delegates,
                  ...context.localizationDelegates,
                ],
                supportedLocales: context.supportedLocales,
                locale: context.locale,
                builder: (_, child) => GlobalLoadingOverlay(child: child!),
                home: const Scaffold(),
                initialRoute: '/data',
                routes: {
                  '/data': (_) => GlobalPopScope(
                    child: ChangeNotifierProvider<GpsManager>.value(
                      value: gpsManager,
                      child: const DataManagementSettingsPage(),
                    ),
                  ),
                },
              ),
            ),
          ),
        );
        await tester.pump(const Duration(seconds: 1));
      });
      await tester.pumpAndSettle();

      Future<void> tap(String key) async {
        final target = find.text(tr(key));
        await tester.ensureVisible(target);
        await tester.pumpAndSettle();
        await tester.tap(target);
        await tester.pumpAndSettle();
      }

      Future<void> openPicker(bool? available) async {
        backend.hasRawData = available;
        await tap('data.export_data.export_all');
        expect(
          find.text(tr('data.export_data.raw_data_group')),
          available == false ? findsNothing : findsOneWidget,
        );
        expect(
          find.text(tr('data.export_data.include_raw_data')),
          findsOneWidget,
        );
        expect(find.text(tr('data.export_data.format_mldx')), findsOneWidget);
      }

      // Attachments, including header-only records, are preserved by default
      // in MLDX, while still allowing users to opt out explicitly.
      for (final includeRawData in [true, false]) {
        await openPicker(true);
        if (!includeRawData) {
          await tester.ensureVisible(find.byType(Switch));
          await tester.pumpAndSettle();
          await tester.tap(find.byType(Switch));
          await tester.pumpAndSettle();
        }
        await tap('common.export');
        expect(backend.archivedRawData, includeRawData);
        await tap('common.ok');
      }
      for (final format in RawDataExportType.values) {
        final exportButton = tester.getCenter(
          find.text(tr('data.export_data.export_all')),
        );
        await openPicker(true);
        await tap('data.export_data.raw_data_group');
        await tap('journey.export_raw_data_${format.name}');
        expect(
          find.text(tr('journey.raw_data_lossy_format_warning')),
          format == RawDataExportType.csv ? findsNothing : findsOneWidget,
        );
        final fail = format == RawDataExportType.kml;
        final pending = Completer<api.ExportResult>();
        backend.pendingExport = pending;
        final rawDataChecks = backend.rawDataChecks;
        final loading = GlobalLoadingManager.instance;

        await tester.tap(find.text(tr('common.export')));
        await tester.pump();
        expect(backend.exportedFormat, format);
        expect(loading.isNavigationBlocked, isTrue);
        expect(loading.isLoading, isFalse);
        expect(
          find.byWidgetPredicate(
            (widget) =>
                widget is ModalBarrier &&
                !widget.dismissible &&
                widget.color == Colors.transparent,
          ),
          findsOneWidget,
        );

        // The barrier protects the page before the delayed spinner appears.
        await tester.pump(const Duration(milliseconds: 180));
        await tester.tapAt(exportButton);
        await navigatorKey.currentState!.maybePop();
        await tester.pump();
        expect(backend.rawDataChecks, rawDataChecks);
        expect(find.byType(DataManagementSettingsPage), findsOneWidget);
        expect(loading.isLoading, isFalse);

        await tester.pump(const Duration(milliseconds: 100));
        expect(loading.isLoading, isTrue);
        if (fail) {
          pending.completeError(StateError('Export failed'));
        } else {
          pending.complete(api.ExportResult.dataIsEmpty);
        }
        await tester.pumpAndSettle();
        expect(loading.isNavigationBlocked, isFalse);
        expect(loading.isLoading, isFalse);
        expect(
          find.text(
            tr(
              fail
                  ? 'data.export_data.error.unexpected'
                  : 'data.export_data.error.no_raw_data_to_export',
            ),
          ),
          findsOneWidget,
        );
        await tap('common.ok');
        backend.pendingExport = null;
      }

      await openPicker(false);
      await tap('common.cancel');
      // An inspection failure must still offer a complete MLDX backup.
      await openPicker(null);
      await tap('common.export');
      expect(backend.archivedRawData, isTrue);
      await tap('common.ok');
      await navigatorKey.currentState!.maybePop();
      await tester.pumpAndSettle();
      expect(find.byType(DataManagementSettingsPage), findsNothing);
    },
  );
}

class _ExportApi extends Fake implements RustLibApi {
  bool? hasRawData = false;
  RawDataExportType? exportedFormat;
  bool? archivedRawData;
  int rawDataChecks = 0;
  Completer<api.ExportResult>? pendingExport;

  @override
  Future<bool> crateApiApiHasJourneys() async => true;

  @override
  Future<bool> crateApiApiHasJourneysRawData() async {
    rawDataChecks++;
    return hasRawData ?? (throw StateError('Raw-data inspection failed'));
  }

  @override
  Future<api.ExportResult> crateApiApiExportAllJourneysRawData({
    required String targetFilepath,
    required RawDataExportType exportType,
  }) async {
    exportedFormat = exportType;
    return pendingExport == null
        ? api.ExportResult.dataIsEmpty
        : await pendingExport!.future;
  }

  @override
  Future<api.ExportResult> crateApiApiGenerateFullArchive({
    required String targetFilepath,
    required bool includeRawData,
  }) async {
    archivedRawData = includeRawData;
    return api.ExportResult.dataIsEmpty;
  }

  @override
  Future<List<LegacyRawDataFile>> crateApiApiListAllLegacyRawData() async => [];
}

class _TestGpsManager extends ChangeNotifier implements GpsManager {
  @override
  GpsRecordingStatus get recordingStatus => GpsRecordingStatus.none;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _TestWakelockPlatform extends WakelockPlusMacOSPlugin {
  @override
  Future<void> toggle({required bool enable}) async {}
}
