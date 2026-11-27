import 'dart:ffi';

import 'package:easy_localization/easy_localization.dart';
// MMKV exposes its platform fake through transitive packages.
// ignore: depend_on_referenced_packages
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:logging/logging.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/permission_request_sheet.dart';
import 'package:memolanes/common/component/setup_tile.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:memolanes/utils/nav_helper.dart' as navigation;
// ignore: depend_on_referenced_packages
import 'package:mmkv_platform_interface/mmkv_platform_interface.dart';
import 'package:permission_handler/permission_handler.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final preferences = _Preferences();
  for (final target in [TargetPlatform.iOS, TargetPlatform.android]) {
    group(target.name, () => _permissionTests(target, preferences));
  }
}

void _permissionTests(TargetPlatform target, _Preferences preferences) {
  const permissionChannel = MethodChannel(
    'flutter.baseflow.com/permissions/methods',
  );
  const geolocatorChannel = MethodChannel('flutter.baseflow.com/geolocator');
  const preferencesChannel = MethodChannel(
    'plugins.flutter.io/shared_preferences',
  );
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final service = PermissionService();
  final motionPermission = target == TargetPlatform.android
      ? Permission.activityRecognition
      : Permission.sensors;
  final checked = <int>[];
  final requested = <int>[];
  var location = PermissionStatus.granted;
  var locationStatusUnavailable = false;
  var motion = PermissionStatus.denied;
  var motionResult = PermissionStatus.denied;
  var notification = PermissionStatus.granted;

  setUpAll(() async {
    MMKVPluginPlatform.instance = preferences;
    await MMKVUtil.init();
    messenger.setMockMethodCallHandler(
      preferencesChannel,
      (call) async => call.method == 'getAll' ? <String, Object>{} : null,
    );
    await EasyLocalization.ensureInitialized();
  });
  tearDownAll(() {
    messenger.setMockMethodCallHandler(preferencesChannel, null);
  });
  setUp(() {
    preferences.values.clear();
    preferences.failedKey = null;
    checked.clear();
    requested.clear();
    location = PermissionStatus.granted;
    locationStatusUnavailable = false;
    motion = PermissionStatus.denied;
    motionResult = PermissionStatus.denied;
    notification = PermissionStatus.granted;
    debugDefaultTargetPlatformOverride = target;
    messenger.setMockMethodCallHandler(geolocatorChannel, (call) async {
      expect(call.method, 'isLocationServiceEnabled');
      return true;
    });
    messenger.setMockMethodCallHandler(permissionChannel, (call) async {
      switch (call.method) {
        case 'checkPermissionStatus':
          final permission = call.arguments as int;
          checked.add(permission);
          if (permission == Permission.location.value) {
            if (locationStatusUnavailable) {
              throw PlatformException(code: 'location_status_unavailable');
            }
            return location.index;
          }
          if (permission == motionPermission.value) return motion.index;
          if (permission == Permission.notification.value) {
            return notification.index;
          }
          return PermissionStatus.granted.index;
        case 'requestPermissions':
          final permissions = (call.arguments as List).cast<int>();
          expect(permissions, [motionPermission.value]);
          requested.addAll(permissions);
          motion = motionResult;
          return {motionPermission.value: motion.index};
        default:
          throw UnsupportedError('Unexpected permission call: ${call.method}');
      }
    });
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(permissionChannel, null);
    messenger.setMockMethodCallHandler(geolocatorChannel, null);
    debugDefaultTargetPlatformOverride = null;
  });

  test('uses the platform motion permission', () {
    expect(service.motionPermission, motionPermission);
  });
  for (final platform in [TargetPlatform.iOS, TargetPlatform.android]) {
    test(
      'keeps location usable after permission setup on ${platform.name}',
      () async {
        debugDefaultTargetPlatformOverride = platform;
        final expectedRequests = [
          Permission.location.value,
          if (platform == TargetPlatform.iOS) Permission.locationAlways.value,
        ];
        var location = PermissionStatus.denied;
        messenger.setMockMethodCallHandler(permissionChannel, (call) async {
          switch (call.method) {
            case 'checkPermissionStatus':
              expect(call.arguments, Permission.location.value);
              return location.index;
            case 'requestPermissions':
              final permissions = (call.arguments as List).cast<int>();
              final permission = expectedRequests[requested.length];
              expect(permissions, [permission]);
              requested.add(permission);
              if (permission == Permission.location.value) {
                location = PermissionStatus.granted;
                return {permission: location.index};
              }
              return {permission: PermissionStatus.denied.index};
            default:
              throw UnsupportedError(
                'Unexpected permission call: ${call.method}',
              );
          }
        });

        expect(await service.checkLocationPermission(), isFalse);
        expect(await service.runLocationRequest(), isEmpty);
        expect(requested, expectedRequests);
        expect(await service.checkLocationPermission(), isTrue);
      },
    );
  }

  test(
    'motion denial stays optional and is not automatically offered again',
    () async {
      final initial = await service.readPermissionSnapshot();
      expect(initial.motion.granted, isFalse);
      expect(initial.motion.denied, isFalse);
      expect(await service.needAnyPermission(), isTrue);
      expect(requested, isEmpty);

      final effects = await service.runMotionRequest();
      expect(
        effects.single.messageTrKey,
        'permission_sheet.motion_permission_denied',
      );
      expect(effects.single.openAppSettings, isFalse);
      expect(preferences.values[MMKVKey.requestedMotion], isTrue);
      final denied = await service.readPermissionSnapshot();
      expect(denied.motion.denied, isTrue);
      expect(denied.location.granted, isTrue);
      expect(await service.checkLocationPermission(), isTrue);
      expect(await service.needAnyPermission(), isFalse);
      expect(await service.needAnyPermission(), isFalse);
      expect(requested, [motionPermission.value]);

      motionResult = PermissionStatus.granted;
      expect(await service.runMotionRequest(), isEmpty);
      expect((await service.readPermissionSnapshot()).motion.granted, isTrue);
      expect(requested, [motionPermission.value, motionPermission.value]);
    },
  );

  test('an existing motion grant never requests permission again', () async {
    motion = PermissionStatus.granted;
    expect((await service.readPermissionSnapshot()).motion.granted, isTrue);
    expect(await service.needAnyPermission(), isFalse);
    expect(await service.runMotionRequest(), isEmpty);
    expect(requested, isEmpty);
  });

  test(
    'permanently denied motion points to settings without another request',
    () async {
      motion = PermissionStatus.permanentlyDenied;
      expect(
        (await service.readPermissionSnapshot()).motion.permanentlyDenied,
        isTrue,
      );
      expect(await service.needAnyPermission(), isFalse);
      final effects = await service.runMotionRequest();
      expect(
        effects.single.messageTrKey,
        'permission_sheet.motion_permission_denied',
      );
      expect(effects.single.openAppSettings, isTrue);
      expect(requested, isEmpty);
      expect(await service.checkLocationPermission(), isTrue);
    },
  );

  test(
    'an unsaved motion request marker cannot trigger an untracked prompt',
    () async {
      preferences.failedKey = MMKVKey.requestedMotion;
      await expectLater(service.runMotionRequest(), throwsStateError);
      expect(requested, isEmpty);
      expect(preferences.values[MMKVKey.requestedMotion], isNull);
    },
  );

  testWidgets(
    'the unified sheet requests optional motion only after Enable All',
    (tester) async {
      try {
        await _pumpPermissionHost(tester);
        await tester.tap(find.text('Permissions'));
        await tester.pumpAndSettle();
        expect(find.text(tr('permission_sheet.motion_title')), findsOneWidget);
        expect(requested, isEmpty);

        await tester.tap(find.text(tr('permission_sheet.enable_all')));
        await tester.pumpAndSettle();
        expect(requested, [motionPermission.value]);
        expect(
          find.text(tr('permission_sheet.motion_permission_denied')),
          findsOneWidget,
        );
        await tester.tap(find.text(tr('common.ok')));
        await tester.pumpAndSettle();
        expect(find.text(tr('permission_sheet.title')), findsNothing);
        expect(await service.checkLocationPermission(), isTrue);
        expect(await service.needAnyPermission(), isFalse);

        await tester.tap(find.text('Permissions'));
        await tester.pumpAndSettle();
        expect(find.text(tr('permission_sheet.motion_title')), findsOneWidget);
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.resumed,
        );
        await tester.pump(const Duration(milliseconds: 250));
        await tester.pumpAndSettle();
        expect(requested, [motionPermission.value]);
        await tester.tap(find.text(tr('permission_sheet.skip')));
        await tester.pumpAndSettle();
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    },
  );

  if (target == TargetPlatform.android) {
    testWidgets('recording requires an explicit background location grant', (
      tester,
    ) async {
      try {
        final records = <LogRecord>[];
        final subscription = Logger('App').onRecord.listen(records.add);
        addTearDown(subscription.cancel);
        var background = PermissionStatus.denied;
        var openedSettings = 0;
        messenger.setMockMethodCallHandler(permissionChannel, (call) async {
          switch (call.method) {
            case 'checkPermissionStatus':
              expectSync(call.arguments, Permission.locationAlways.value);
              return background.index;
            case 'openAppSettings':
              openedSettings++;
              return true;
            default:
              throw UnsupportedError(
                'Unexpected permission call: ${call.method}',
              );
          }
        });
        await _pumpPermissionHost(
          tester,
          navigatorKey: navigation.navigatorKey,
        );
        final result = navigation.checkAndRequestBackgroundLocationPermission();
        await tester.pumpAndSettle();
        expect(
          find.text(tr('location_service.always_location_required')),
          findsOneWidget,
        );
        await tester.tap(
          find.text(tr('permission_sheet.open_system_settings')),
        );
        await tester.pumpAndSettle();

        final allowed = await result;
        expect(
          allowed,
          isFalse,
          reason: records
              .map((record) => '${record.message}\n${record.stackTrace}')
              .join('\n'),
        );
        expect(openedSettings, 1);
        expect(requested, isEmpty);
        background = PermissionStatus.granted;
        expect(
          await navigation.checkAndRequestBackgroundLocationPermission(),
          isTrue,
        );
      } finally {
        debugDefaultTargetPlatformOverride = null;
      }
    });
  }

  for (final statusUnavailable in [false, true]) {
    testWidgets(
      'shows a motion grant with ${statusUnavailable ? 'unavailable' : 'denied'} location',
      (tester) async {
        try {
          location = PermissionStatus.permanentlyDenied;
          locationStatusUnavailable = statusUnavailable;
          motionResult = PermissionStatus.granted;
          await _pumpPermissionHost(tester);
          await tester.tap(find.text('Permissions'));
          await tester.pumpAndSettle();

          final motionTitle = tr('permission_sheet.motion_title');
          await tester.tap(find.text(motionTitle));
          await tester.pumpAndSettle();

          final check = find.byIcon(Icons.check_circle);
          expect(
            find.descendant(
              of: find.widgetWithText(SetupTile, motionTitle),
              matching: check,
            ),
            findsOneWidget,
          );
          expect(await service.checkLocationPermission(), isFalse);
          expect(requested, [motionPermission.value]);
          await tester.tap(find.text(tr('permission_sheet.skip')));
          await tester.pumpAndSettle();
        } finally {
          debugDefaultTargetPlatformOverride = null;
        }
      },
    );
  }
}

Future<void> _pumpPermissionHost(
  WidgetTester tester, {
  GlobalKey<NavigatorState>? navigatorKey,
}) async {
  const locale = Locale('en', 'US');
  await tester.runAsync(() async {
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: const [locale],
        path: 'assets/translations',
        startLocale: locale,
        saveLocale: false,
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
            home: Builder(
              builder: (context) => Scaffold(
                body: TextButton(
                  onPressed: () => showPermissionRequestSheet(context),
                  child: const Text('Permissions'),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 100));
  });
  await tester.pumpAndSettle();
}

class _Preferences extends MMKVPluginPlatform {
  final values = <String, bool>{};
  String? failedKey;

  @override
  Future<String> getApplicationDocumentsPath() async => '/test';

  @override
  Future<String> initialize(
    String rootDir, {
    String? groupDir,
    int logLevel = 1,
    Pointer<NativeFunction<LogCallbackWrap>>? logHandler,
  }) async => rootDir;

  @override
  Pointer<Void> Function(int, Pointer<Utf8>, int, int, int, int, int, int, int)
  getDefaultMMKVFunc() =>
      (_, _, _, _, _, _, _, _, _) => Pointer<Void>.fromAddress(1);

  @override
  int Function(Pointer<Void>, Pointer<Utf8>, int) encodeBoolFunc() =>
      (_, key, value) {
        final name = key.toDartString();
        if (name == failedKey) return 0;
        values[name] = value != 0;
        return 1;
      };

  @override
  int Function(Pointer<Void>, Pointer<Utf8>, int) decodeBoolFunc() =>
      (_, key, fallback) =>
          (values[key.toDartString()] ?? (fallback != 0)) ? 1 : 0;
}
