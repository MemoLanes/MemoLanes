import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/body/settings/settings_category_pages.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/gps_manager.dart';
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
    'export dialog refreshes raw-data availability and routes formats',
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
                theme: AppTheme.light,
                localizationsDelegates: [
                  ...GlobalMaterialLocalizations.delegates,
                  ...context.localizationDelegates,
                ],
                supportedLocales: context.supportedLocales,
                locale: context.locale,
                home: ChangeNotifierProvider<GpsManager>.value(
                  value: gpsManager,
                  child: const DataManagementSettingsPage(),
                ),
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
        backend.hasRawPoints = available;
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

      // Header-only attachments have no GPS points; preserve them by default
      // in MLDX, while still allowing users to opt out explicitly.
      for (final includeRawData in [true, false]) {
        await openPicker(false);
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
        await openPicker(true);
        await tap('data.export_data.raw_data_group');
        await tap('journey.export_raw_data_${format.name}');
        expect(
          find.text(tr('journey.raw_data_lossy_format_warning')),
          format == RawDataExportType.csv ? findsNothing : findsOneWidget,
        );
        await tap('common.export');
        expect(backend.exportedFormat, format);
        expect(
          find.text(tr('data.export_data.error.no_raw_data_to_export')),
          findsOneWidget,
        );
        await tap('common.ok');
      }
      await openPicker(false);
      await tap('common.cancel');
      // An inspection failure must still offer a complete MLDX backup.
      await openPicker(null);
      await tap('common.export');
      expect(backend.archivedRawData, isTrue);
    },
  );
}

class _ExportApi extends Fake implements RustLibApi {
  bool? hasRawPoints = false;
  RawDataExportType? exportedFormat;
  bool? archivedRawData;

  @override
  Future<bool> crateApiApiHasJourneys() async => true;

  @override
  Future<bool> crateApiApiHasJourneysRawData() async =>
      hasRawPoints ?? (throw StateError('Raw-data inspection failed'));

  @override
  Future<api.ExportResult> crateApiApiExportAllJourneysRawData({
    required String targetFilepath,
    required RawDataExportType exportType,
  }) async {
    exportedFormat = exportType;
    return api.ExportResult.dataIsEmpty;
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
