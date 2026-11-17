import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/settings/debug/tracelet_motion_diagnostics.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:tracelet/tracelet.dart' as tl;
// ignore: depend_on_referenced_packages
import 'package:tracelet_platform_interface/tracelet_platform_interface.dart'
    as platform;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    PackageInfo.setMockInitialValues(
      appName: 'MemoLanes',
      packageName: 'com.memolanes.test',
      version: '0.0.1',
      buildNumber: '4',
      buildSignature: '',
    );
  });
  tearDown(() => debugDefaultTargetPlatformOverride = null);

  test(
    'preserves the motion timeline after capture stops and reopens',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'tracelet-motion-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final diagnostics = TraceletMotionDiagnostics(() async => directory);
      addTearDown(() async {
        await diagnostics.stopCapture();
        diagnostics.dispose();
      });

      void receiveFix(int timestampMs, bool isMoving) {
        diagnostics.locationReceived(
          LocationData(
            latitude: 12.345678,
            longitude: 98.765432,
            timestampMs: timestampMs,
            accuracy: 5,
            speed: isMoving ? 1.5 : 0,
          ),
          isMovingAtFix: isMoving,
          source: isMoving ? 'gps' : 'network',
          isMock: false,
          reducedAccuracy: !isMoving,
          speedAccuracyMps: isMoving ? 0.25 : null,
          locationEvent: isMoving ? null : 'periodic',
        );
      }

      const firstFix = 1760000000000;
      diagnostics.recordingStarted(true);
      receiveFix(firstFix, true);
      expect(await diagnostics.savedLogs(), isEmpty);

      await diagnostics.startCapture();
      diagnostics.didChangeAppLifecycleState(AppLifecycleState.paused);
      receiveFix(firstFix + 1000, true);
      diagnostics.motionChanged(false);
      receiveFix(firstFix + 61000, false);
      expect(diagnostics.isMoving, false);
      await diagnostics.stopCapture();
      diagnostics.recordingStopped();
      expect(diagnostics.isMoving, isNull);
      expect(diagnostics.lastFix, isNull);

      final reopened = TraceletMotionDiagnostics(() async => directory);
      addTearDown(reopened.dispose);
      final text = await (await reopened.savedLogs()).single.readAsString();
      final rows = _rows(text);
      expect(rows.map((row) => row['type']), [
        'capture_started',
        'app_lifecycle',
        'location',
        'motion_changed',
        'location',
        'capture_stopped',
      ]);
      expect(rows[0]['isMoving'], true);
      expect(rows[0]['schema'], 2);
      expect(rows[2]['speedAccuracyMps'], 0.25);
      expect(rows[3]['isMoving'], false);
      expect(rows[4]['fixIntervalMs'], 60000);
      expect(rows[4]['isMovingAtFix'], false);
      expect(rows[4]['source'], 'network');
      expect(rows[4]['reducedAccuracy'], true);
      expect(rows[4]['speedAccuracyMps'], isNull);
      expect(rows[4]['locationEvent'], 'periodic');
      _expectNoCoordinates(text);
    },
  );

  for (final target in [TargetPlatform.iOS, TargetPlatform.android]) {
    group('independent SDK observation on ${target.name}', () {
      const nativeChannel = MethodChannel(
        'com.memolanes/tracelet_motion_diagnostics',
      );
      const permissionChannel = MethodChannel(
        'flutter.baseflow.com/permissions/methods',
      );
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      late platform.TraceletPlatform previousPlatform;
      late _MotionPlatform sdk;
      late Directory directory;
      late TraceletMotionDiagnostics diagnostics;
      final nativeCalls = <MethodCall>[];
      final permissionCalls = <MethodCall>[];
      final businessSubscriptions = <StreamSubscription<dynamic>>[];

      setUp(() async {
        debugDefaultTargetPlatformOverride = target;
        sdk = _MotionPlatform();
        previousPlatform = platform.TraceletPlatform.instance;
        platform.TraceletPlatform.instance = sdk;
        directory = await Directory.systemTemp.createTemp(
          'tracelet-motion-sdk-',
        );
        diagnostics = TraceletMotionDiagnostics(() async => directory);
        nativeCalls.clear();
        permissionCalls.clear();
        businessSubscriptions.clear();
        messenger.setMockMethodCallHandler(nativeChannel, (call) async {
          nativeCalls.add(call);
          expect(call.method, 'snapshot');
          expect((call.arguments as Map).keys, ['fixAtMs']);
          return <String, Object?>{
            'available': true,
            'accelerometerMoving': false,
            'speedMoving': true,
            'speedState': 'moving',
            'motionFixAtMs': (call.arguments as Map)['fixAtMs'],
            'motionSpeedMps': 0.75,
            'motionSpeedSource': 'position_delta',
            'motionSpeedReason': 'valid',
            'rawFixMatched': (call.arguments as Map)['fixAtMs'] != null,
            'rawSpeedMps': 1.5,
          };
        });
        messenger.setMockMethodCallHandler(permissionChannel, (call) async {
          permissionCalls.add(call);
          throw StateError('Diagnostics must not request or check permissions');
        });
      });

      tearDown(() async {
        await diagnostics.stopCapture();
        await diagnostics.detach();
        diagnostics.dispose();
        for (final subscription in businessSubscriptions) {
          await subscription.cancel();
        }
        // Reset SDK caches only after the isolation assertions have completed.
        tl.Tracelet.removeListeners();
        await sdk.close();
        platform.TraceletPlatform.instance = previousPlatform;
        messenger.setMockMethodCallHandler(nativeChannel, null);
        messenger.setMockMethodCallHandler(permissionChannel, null);
        await directory.delete(recursive: true);
      });

      test(
        'detaches without capture without removing business listeners',
        () async {
          final businessFixes = <tl.Location>[];
          businessSubscriptions.add(tl.Tracelet.onLocation(businessFixes.add));
          await diagnostics.attach();
          sdk.locations.add(_fix(1760000000000, true));
          await _deliverEvents();
          expect(diagnostics.fixCount, 1);

          await diagnostics.detach();
          expect(sdk.enabled.hasListener, false);
          expect(sdk.speedMotion.hasListener, false);
          sdk.locations.add(_fix(1760000001000, true));
          sdk.enabled.add(true);
          await _deliverEvents();
          expect(businessFixes, hasLength(2));
          expect(diagnostics.fixCount, 0);
          expect(diagnostics.isMoving, isNull);
          expect(sdk.stateReads, 1);
          expect(diagnostics.recentLines, isEmpty);
          expect(nativeCalls, isEmpty);
          expect(sdk.mutations, isEmpty);
          expect(permissionCalls, isEmpty);
        },
      );

      test(
        'keeps capturing after detach without disturbing recording',
        () async {
          final businessFixes = <tl.Location>[];
          final businessMotion = <bool>[];
          businessSubscriptions.add(tl.Tracelet.onLocation(businessFixes.add));
          businessSubscriptions.add(
            tl.Tracelet.onMotionChange((location) {
              businessMotion.add(location.isMoving);
            }),
          );

          await diagnostics.attach();
          expect(sdk.stateReads, 1);
          expect(diagnostics.isMoving, true);
          expect(diagnostics.fixCount, 0);
          expect(sdk.enabled.hasListener, true);
          expect(sdk.speedMotion.hasListener, false);

          await diagnostics.startCapture();
          await diagnostics.detach();
          expect(diagnostics.capturing, true);
          expect(sdk.enabled.hasListener, true);
          expect(sdk.speedMotion.hasListener, true);

          const firstFix = 1760000000000;
          sdk.locations.add(_fix(firstFix, true));
          await _deliverEvents();
          sdk.motion.add(_fix(firstFix + 1000, false));
          sdk.speedMotion.add(_stationaryTransition());
          sdk.locations.add(_fix(firstFix + 60000, false));
          await _deliverEvents();
          expect(diagnostics.fixCount, 2);
          expect(diagnostics.lastFix?.fixIntervalMs, 60000);
          expect(diagnostics.isMoving, false);
          expect(businessFixes, hasLength(2));
          expect(businessMotion, [false]);

          await diagnostics.stopCapture();
          expect(diagnostics.capturing, false);
          expect(diagnostics.writeError, isNull);
          expect(sdk.enabled.hasListener, false);
          expect(sdk.speedMotion.hasListener, false);
          final recentLines = diagnostics.recentLines;
          final snapshotCount = nativeCalls.length;
          final stateReads = sdk.stateReads;
          final file = (await diagnostics.savedLogs()).single;
          final text = await file.readAsString();
          final rows = _rows(text);
          expect(rows.first['type'], 'capture_started');
          expect(rows.first['isMoving'], true);
          expect(rows.last['type'], 'capture_stopped');
          expect(rows.where((row) => row['type'] == 'location'), hasLength(2));
          final firstNativeMotion =
              rows.firstWhere(
                    (row) => row['type'] == 'location',
                  )['nativeMotion']
                  as Map<String, Object?>;
          expect(firstNativeMotion, containsPair('accelerometerMoving', false));
          expect(firstNativeMotion, containsPair('speedMoving', true));
          expect(firstNativeMotion, containsPair('speedState', 'moving'));
          expect(firstNativeMotion, containsPair('motionFixAtMs', firstFix));
          expect(firstNativeMotion, containsPair('motionSpeedMps', 0.75));
          expect(
            firstNativeMotion,
            containsPair('motionSpeedSource', 'position_delta'),
          );
          expect(firstNativeMotion, containsPair('motionSpeedReason', 'valid'));
          expect(firstNativeMotion, containsPair('sdkSpeedMps', 1.5));
          expect(firstNativeMotion, containsPair('sdkSpeedSource', 'platform'));
          expect(
            rows.singleWhere((row) => row['type'] == 'speed_motion_changed'),
            containsPair('speedState', 'stationary'),
          );
          expect(nativeCalls, isNotEmpty);
          _expectNoCoordinates(text);

          sdk.locations.add(_fix(firstFix + 120000, true));
          sdk.motion.add(_fix(firstFix + 120000, true));
          sdk.enabled.add(true);
          sdk.speedMotion.add(_stationaryTransition());
          await _deliverEvents();
          expect(diagnostics.fixCount, 0);
          expect(diagnostics.lastFix, isNull);
          expect(diagnostics.isMoving, isNull);
          expect(diagnostics.recentLines, recentLines);
          expect(await file.readAsString(), text);
          expect(nativeCalls, hasLength(snapshotCount));
          expect(sdk.stateReads, stateReads);
          expect(businessFixes, hasLength(3));
          expect(businessMotion, [false, true]);
          expect(sdk.mutations, isEmpty);
          expect(permissionCalls, isEmpty);
        },
      );

      test(
        'releases its subscriptions when the log directory cannot open',
        () async {
          final businessFixes = <tl.Location>[];
          businessSubscriptions.add(tl.Tracelet.onLocation(businessFixes.add));
          await File('${directory.path}/tracelet_motion_logs')
              .writeAsString('file');

          await expectLater(
            diagnostics.startCapture(),
            throwsA(isA<FileSystemException>()),
          );
          expect(diagnostics.capturing, false);
          expect(sdk.stateReads, 1);
          expect(sdk.enabled.hasListener, false);
          expect(sdk.speedMotion.hasListener, false);

          sdk.locations.add(_fix(1760000000000, true));
          sdk.motion.add(_fix(1760000000000, true));
          sdk.enabled.add(true);
          sdk.speedMotion.add(_stationaryTransition());
          await _deliverEvents();
          expect(diagnostics.fixCount, 0);
          expect(diagnostics.lastFix, isNull);
          expect(diagnostics.isMoving, isNull);
          expect(diagnostics.recentLines, isEmpty);
          expect(sdk.stateReads, 1);
          expect(businessFixes, hasLength(1));
          expect(nativeCalls, isEmpty);
          expect(sdk.mutations, isEmpty);
          expect(permissionCalls, isEmpty);
        },
      );
    });
  }
}

List<Map<String, Object?>> _rows(String text) => const LineSplitter()
    .convert(text)
    .map((line) => jsonDecode(line) as Map<String, Object?>)
    .toList();

void _expectNoCoordinates(String text) {
  expect(text, isNot(contains('12.345678')));
  expect(text, isNot(contains('98.765432')));
  expect(text, isNot(contains('latitude')));
  expect(text, isNot(contains('longitude')));
}

Future<void> _deliverEvents() => Future<void>.delayed(Duration.zero);

platform.TlLocation _fix(int timestampMs, bool isMoving) => platform.TlLocation(
  coords: platform.TlCoords(
    latitude: 12.345678,
    longitude: 98.765432,
    accuracy: 5,
    speed: isMoving ? 1.5 : 0,
    heading: 0,
    altitude: 0,
    altitudeAccuracy: 0,
    speedAccuracy: 0.25,
    headingAccuracy: 0,
  ),
  battery: platform.TlBattery(level: 1, isCharging: false),
  timestamp: DateTime.fromMillisecondsSinceEpoch(
    timestampMs,
    isUtc: true,
  ).toIso8601String(),
  uuid: 'fix-$timestampMs',
  isMoving: isMoving,
  odometer: 0,
  extras: {'locationSource': 'gps'},
);

platform.TlSpeedMotionEvent _stationaryTransition() =>
    platform.TlSpeedMotionEvent(
      state: platform.TlSpeedMotionState.stationary,
      previousState: platform.TlSpeedMotionState.moving,
      trackingMode: platform.TlTrackingMode.periodic,
    );

class _MotionPlatform extends platform.TraceletPlatform {
  final locations = StreamController<platform.TlLocation>.broadcast();
  final motion = StreamController<platform.TlLocation>.broadcast();
  final enabled = StreamController<bool>.broadcast();
  final speedMotion = StreamController<platform.TlSpeedMotionEvent>.broadcast();
  final mutations = <String>[];
  int stateReads = 0;

  @override
  Stream<platform.TlLocation> get locationEvents => locations.stream;
  @override
  Stream<platform.TlLocation> get motionChangeEvents => motion.stream;
  @override
  Stream<bool> get enabledChangeEvents => enabled.stream;
  @override
  Stream<platform.TlSpeedMotionEvent> get motionModeChangeEvents =>
      speedMotion.stream;

  @override
  Future<Map<String, Object?>> getState() async {
    stateReads++;
    return {'enabled': true, 'isMoving': true};
  }

  Never _unexpected(String operation) {
    mutations.add(operation);
    throw StateError('Diagnostics must not call $operation');
  }

  @override
  Future<Map<String, Object?>> ready(platform.TlConfig config) async =>
      _unexpected('ready');
  @override
  Future<Map<String, Object?>> setConfig(platform.TlConfig config) async =>
      _unexpected('setConfig');
  @override
  Future<Map<String, Object?>> start() async => _unexpected('start');
  @override
  Future<Map<String, Object?>> stop() async => _unexpected('stop');
  @override
  Future<platform.AuthorizationStatus> requestLocationAuthorization() async =>
      _unexpected('requestLocationAuthorization');
  @override
  Future<platform.MotionAuthorizationStatus>
  requestMotionAuthorization() async =>
      _unexpected('requestMotionAuthorization');
  @override
  Future<platform.NotificationAuthorizationStatus>
  requestNotificationAuthorization() async =>
      _unexpected('requestNotificationAuthorization');

  Future<void> close() async {
    await locations.close();
    await motion.close();
    await enabled.close();
    await speedMotion.close();
  }
}
