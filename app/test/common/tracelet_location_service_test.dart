import 'dart:async';
import 'dart:ffi';

// MMKV exposes its platform fake through transitive packages.
// ignore: depend_on_referenced_packages
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';
import 'package:tracelet/tracelet.dart'
    show Tracelet, MotionAuthorizationStatus;
import 'package:memolanes/common/service/location/tracelet_location_service.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/recording/location_recording.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/common/service/location/location_service_factory.dart';
import 'package:permission_handler/permission_handler.dart';
// ignore: depend_on_referenced_packages
import 'package:mmkv_platform_interface/mmkv_platform_interface.dart';
import 'package:memolanes/src/rust/frb_generated.dart' as app_rust;
import 'package:memolanes/src/rust/gps.dart' show ExtendedRawGPSPoint;
// Tracelet.ready() constructs a Rust trip manager even in a queue-only test.
// ignore: depend_on_referenced_packages, implementation_imports
import 'package:tracelet_platform_interface/src/rust/api_dart/trip.dart';
// ignore: depend_on_referenced_packages, implementation_imports
import 'package:tracelet_platform_interface/src/rust/frb_generated.dart';
// ignore: depend_on_referenced_packages
import 'package:tracelet_platform_interface/tracelet_platform_interface.dart'
    show
        TlConfig,
        TlCurrentPositionOptions,
        TlLocation,
        TlCoords,
        TlBattery,
        TlAuthorizationRequest,
        TlStationaryTrackingMode,
        TlMotionDetectionMode,
        TlDesiredAccuracy,
        TraceletPlatform;

const _pendingQueueKey = 'TraceletLocationService.recordingQueuePending';

const _first = '00000000-0000-4000-8000-000000000001';
const _invalid = '00000000-0000-4000-8000-000000000002';
const _second = '00000000-0000-4000-8000-000000000003';
const _third = '00000000-0000-4000-8000-000000000004';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final preferences = _Preferences();
  const permissionChannel = MethodChannel(
    'flutter.baseflow.com/permissions/methods',
  );
  const geolocatorChannel = MethodChannel('flutter.baseflow.com/geolocator');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  var permission = PermissionStatus.granted;
  var motionPermission = PermissionStatus.granted;

  setUpAll(() async {
    MMKVPluginPlatform.instance = preferences;
    await MMKVUtil.init();
  });
  setUp(() {
    preferences.values.clear();
    preferences.failedKey = null;
    permission = PermissionStatus.granted;
    motionPermission = PermissionStatus.granted;
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    messenger.setMockMethodCallHandler(geolocatorChannel, (call) async {
      expect(call.method, 'isLocationServiceEnabled');
      return true;
    });
    messenger.setMockMethodCallHandler(permissionChannel, (call) async {
      expect(call.method, 'checkPermissionStatus');
      if (call.arguments == Permission.notification.value) {
        return PermissionStatus.denied.index;
      }
      if (call.arguments == Permission.sensors.value) {
        return motionPermission.index;
      }
      expect(call.arguments, Permission.location.value);
      return permission.index;
    });
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(permissionChannel, null);
    messenger.setMockMethodCallHandler(geolocatorChannel, null);
    debugDefaultTargetPlatformOverride = null;
  });

  test('Tracelet is available only on iOS and can switch to native', () {
    expect(selectedLocationBackend, LocationBackend.tracelet);
    saveLocationBackend(LocationBackend.native);
    expect(selectedLocationBackend, LocationBackend.native);
    saveLocationBackend(LocationBackend.tracelet);
    expect(selectedLocationBackend, LocationBackend.tracelet);
    for (final platform in TargetPlatform.values) {
      debugDefaultTargetPlatformOverride = platform;
      expect(
        supportedLocationBackends.contains(LocationBackend.tracelet),
        platform == TargetPlatform.iOS,
      );
      if (platform != TargetPlatform.iOS) {
        expect(selectedLocationBackend, LocationBackend.native);
        expect(
          () => createLocationService(LocationBackend.tracelet),
          throwsUnsupportedError,
        );
      }
    }
  });

  test('idle initialization and flush do not access native tracking', () async {
    await TraceletLocationService(_recordLocation).initialize();
    await TraceletLocationService(_recordLocation).flushRecording();
    expect(preferences.values, isEmpty);
  });

  test('a failed provider selection write is reported', () {
    preferences.failedKey = 'Settings.useLegacyLocationBackend';
    expect(() => saveLocationBackend(LocationBackend.native), throwsStateError);
    expect(selectedLocationBackend, LocationBackend.tracelet);
  });

  test(
    'Recordings enable activity and heartbeat without changing continuous pace',
    () async {
      RustLib.initMock(api: _TraceletRustApi());
      addTearDown(RustLib.dispose);
      final platform = _HeartbeatTracelet();
      final previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      addTearDown(() {
        Tracelet.removeListeners();
        TraceletPlatform.instance = previousPlatform;
      });
      final service = TraceletLocationService(_recordLocation);
      await service.startLocationUpdates(true);
      expect(platform.config!.app.heartbeatInterval, 60);
      expect(
        platform.config!.ios.locationAuthorizationRequest,
        TlAuthorizationRequest.whenInUse,
      );
      expect(platform.config!.motion.disableMotionActivityUpdates, false);
      expect(platform.motionActivityDisabledOnReady, isFalse);
      expect(
        platform.config!.motion.stationaryTrackingMode,
        TlStationaryTrackingMode.continuous,
      );
      expect(
        platform.config!.motion.stationaryPeriodicAccuracy,
        TlDesiredAccuracy.low,
      );
      expect(platform.config!.motion.speedStationaryDelay, 90);
      expect(platform.config!.motion.speedMovingThreshold, 0.9);
      expect(platform.config!.motion.enableGpsTremorOverride, false);
      expect(platform.config!.motion.speedWakeConfirmCount, 3);
      expect(platform.config!.motion.stationaryExitRadiusMeters, 1000);
      await service.flushRecording();
      await service.stopLocationUpdates();
      await service.flushRecording();
      final map = TraceletLocationService(_recordLocation);
      await map.startLocationUpdates(false);
      expect(platform.config!.app.heartbeatInterval, -1);
      expect(platform.config!.motion.stationaryExitRadiusMeters, 0);
      expect(platform.config!.motion.disableMotionActivityUpdates, true);
      expect(platform.motionRequests, 0);
      await map.stopLocationUpdates();
    },
  );

  test('Recordings start with the selected smart power saving mode', () async {
    RustLib.initMock(api: _TraceletRustApi());
    addTearDown(RustLib.dispose);
    final platform = _HeartbeatTracelet();
    final previousPlatform = TraceletPlatform.instance;
    TraceletPlatform.instance = platform;
    addTearDown(() {
      Tracelet.removeListeners();
      TraceletPlatform.instance = previousPlatform;
    });
    final service = TraceletLocationService(_recordLocation);
    await service.startLocationUpdates(true);
    expect(
      platform.config!.motion.motionDetectionMode,
      TlMotionDetectionMode.smart,
    );
    expect(platform.config!.motion.disableStopDetection, isFalse);

    await service.stopLocationUpdates();
    expect(MMKVUtil.putBool(MMKVKey.smartPowerSavingEnabled, false), isTrue);
    await service.startLocationUpdates(true);
    expect(
      platform.config!.motion.motionDetectionMode,
      TlMotionDetectionMode.accelerometer,
    );
    expect(platform.config!.motion.disableStopDetection, isTrue);
    expect(platform.config!.motion.isMoving, isTrue);

    await service.stopLocationUpdates();
    expect(MMKVUtil.putBool(MMKVKey.smartPowerSavingEnabled, true), isTrue);
    await service.startLocationUpdates(true);
    expect(
      platform.config!.motion.motionDetectionMode,
      TlMotionDetectionMode.smart,
    );
    expect(platform.config!.motion.disableStopDetection, isFalse);
    await service.stopLocationUpdates();
  });

  for (final status in [
    PermissionStatus.permanentlyDenied,
    PermissionStatus.denied,
  ]) {
    test(
      'Motion permission $status falls back without requesting access or blocking recording',
      () async {
        RustLib.initMock(api: _TraceletRustApi());
        addTearDown(RustLib.dispose);
        final previousPlatform = TraceletPlatform.instance;
        final platform = _HeartbeatTracelet();
        motionPermission = status;
        TraceletPlatform.instance = platform;
        addTearDown(() {
          Tracelet.removeListeners();
          TraceletPlatform.instance = previousPlatform;
        });
        final service = TraceletLocationService(_recordLocation);
        await service.startLocationUpdates(true);
        expect(platform.config!.motion.disableMotionActivityUpdates, true);
        expect(platform.motionActivityDisabledOnReady, isTrue);
        expect(platform.config!.app.heartbeatInterval, 60);
        expect(platform.motionRequests, 0);
        expect(platform.enabled, true);
        await service.flushRecording();
        await service.stopLocationUpdates();
      },
    );
  }

  for (final granted in [true, false]) {
    test(
      'cold launch drains the queue before resuming (location=$granted)',
      () async {
        RustLib.initMock(api: _TraceletRustApi());
        addTearDown(RustLib.dispose);
        final backend = _AppRecordingApi();
        app_rust.RustLib.initMock(api: backend);
        addTearDown(app_rust.RustLib.dispose);
        final platform = _TraceletQueue()..enabled = true;
        final previousPlatform = TraceletPlatform.instance;
        TraceletPlatform.instance = platform;
        addTearDown(() {
          Tracelet.removeListeners();
          TraceletPlatform.instance = previousPlatform;
        });
        preferences.values[MMKVKey.isRecording] = true;
        permission = granted
            ? PermissionStatus.granted
            : PermissionStatus.denied;

        await TraceletLocationService(_recordLocation).initialize();

        expect(platform.operations, ['stop', 'ready']);
        expect(platform.enabled, isFalse);
        expect(backend.recorded, hasLength(3));
        expect(platform.rows, isEmpty);
        expect(preferences.values[_pendingQueueKey], isFalse);
        expect(preferences.values[MMKVKey.isRecording], granted);
      },
    );
  }

  test(
    'failed startup preserves queued fixes without resuming next launch',
    () async {
      RustLib.initMock(api: _TraceletRustApi());
      addTearDown(RustLib.dispose);
      final backend = _AppRecordingApi();
      app_rust.RustLib.initMock(api: backend);
      addTearDown(app_rust.RustLib.dispose);
      final platform = _TraceletQueue()..failStart = true;
      final previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      addTearDown(() {
        Tracelet.removeListeners();
        TraceletPlatform.instance = previousPlatform;
      });
      final service = TraceletLocationService(_recordLocation);

      await expectLater(service.startLocationUpdates(true), throwsStateError);
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(preferences.values[_pendingQueueKey], isTrue);
      expect(platform.rows, isNotEmpty);
      platform.operations.clear();

      await TraceletLocationService(_recordLocation).initialize();
      expect(platform.operations, ['stop', 'ready']);
      expect(platform.rows, isEmpty);
      expect(preferences.values[_pendingQueueKey], isFalse);
      expect(backend.recorded, hasLength(3));
    },
  );

  test(
    'failed flush after stopping preserves the queue without restarting',
    () async {
      RustLib.initMock(api: _TraceletRustApi());
      addTearDown(RustLib.dispose);
      final backend = _AppRecordingApi();
      app_rust.RustLib.initMock(api: backend);
      addTearDown(app_rust.RustLib.dispose);
      final platform = _TraceletQueue()..rows.clear();
      final previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      addTearDown(() {
        Tracelet.removeListeners();
        TraceletPlatform.instance = previousPlatform;
      });
      final service = TraceletLocationService(_recordLocation);
      await service.startLocationUpdates(true);
      await service.flushRecording();
      expect(preferences.values[MMKVKey.isRecording], isTrue);
      platform.rows.add(_first);
      backend.failWrites = true;

      await service.stopLocationUpdates();
      expect(backend.recorded, isEmpty);
      await expectLater(service.flushRecording(), throwsStateError);
      expect(platform.enabled, isFalse);
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(preferences.values[_pendingQueueKey], isTrue);
      expect(platform.rows, [_first]);

      backend.failWrites = false;
      await TraceletLocationService(_recordLocation).initialize();
      expect(platform.rows, isEmpty);
      expect(backend.recorded, hasLength(1));
      expect(preferences.values[_pendingQueueKey], isFalse);
      expect(preferences.values[MMKVKey.isRecording], isFalse);
    },
  );

  test(
    'a failed recovery clears recording intent and retains pending fixes',
    () async {
      RustLib.initMock(api: _TraceletRustApi());
      addTearDown(RustLib.dispose);
      final backend = _AppRecordingApi()..failWrites = true;
      app_rust.RustLib.initMock(api: backend);
      addTearDown(app_rust.RustLib.dispose);
      final platform = _TraceletQueue();
      final previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      addTearDown(() {
        Tracelet.removeListeners();
        TraceletPlatform.instance = previousPlatform;
      });
      preferences.values[MMKVKey.isRecording] = true;

      await expectLater(
        TraceletLocationService(_recordLocation).initialize(),
        throwsStateError,
      );
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(preferences.values[_pendingQueueKey], isTrue);
      expect(platform.rows, isNotEmpty);

      backend.failWrites = false;
      await TraceletLocationService(_recordLocation).initialize();
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(preferences.values[_pendingQueueKey], isFalse);
      expect(platform.rows, isEmpty);
      expect(platform.operations, isNot(contains('start')));
    },
  );

  test('a failed pending marker retains the recording recovery clue', () async {
    preferences.values[MMKVKey.isRecording] = true;
    preferences.failedKey = _pendingQueueKey;

    await expectLater(
      TraceletLocationService(_recordLocation).initialize(),
      throwsStateError,
    );
    expect(preferences.values[MMKVKey.isRecording], isTrue);
  });

  group('GpsManager with Tracelet', () {
    late _AppRecordingApi backend;
    late _TraceletQueue platform;
    late TraceletPlatform previousPlatform;
    late GpsManager manager;
    late GeolocatorPlatform previousNativePlatform;
    late _NativeLocationPlatform nativePlatform;
    const notificationChannel = MethodChannel(
      'notification_when_app_is_killed',
    );
    const toastChannel = MethodChannel('PonnamKarthik/fluttertoast');

    setUp(() async {
      RustLib.initMock(api: _TraceletRustApi());
      backend = _AppRecordingApi();
      app_rust.RustLib.initMock(api: backend);
      platform = _TraceletQueue()..rows.clear();
      previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      preferences.values[MMKVKey.isRecording] = true;
      previousNativePlatform = GeolocatorPlatform.instance;
      nativePlatform = _NativeLocationPlatform();
      GeolocatorPlatform.instance = nativePlatform;
      messenger.setMockMethodCallHandler(notificationChannel, (call) async {
        expect(call.method, 'cancelNotificationOnKillService');
        return true;
      });
      messenger.setMockMethodCallHandler(toastChannel, (call) async {
        expect(call.method, 'showToast');
        return true;
      });
      manager = GpsManager();
      await manager.toggleMapTracking(false);
    });
    tearDown(() async {
      manager.dispose();
      await Future<void>.delayed(Duration.zero);
      Tracelet.removeListeners();
      await platform.liveLocations?.close();
      await platform.watchedLocations?.close();
      await nativePlatform.positions.close();
      GeolocatorPlatform.instance = previousNativePlatform;
      TraceletPlatform.instance = previousPlatform;
      app_rust.RustLib.dispose();
      RustLib.dispose();
      for (final channel in [notificationChannel, toastChannel]) {
        messenger.setMockMethodCallHandler(channel, null);
      }
    });

    Future<void> startFollowing() async {
      await manager.changeRecordingState(GpsRecordingStatus.paused);
      platform.watchedLocations = StreamController<TlLocation>.broadcast();
      manager.readyToStart();
      await manager.toggleMapTracking(true);
      expect(platform.watchedLocations!.hasListener, isTrue);
    }

    test(
      'map following switches providers and disconnects the previous stream',
      () async {
        await startFollowing();
        platform.watchedLocations!.add(_traceletFix(2));
        await Future<void>.delayed(Duration.zero);
        expect(manager.latestPosition?.longitude, 2);

        expect(
          await manager.setLocationBackend(LocationBackend.native),
          isTrue,
        );
        expect(manager.locationBackend, LocationBackend.native);
        expect(selectedLocationBackend, LocationBackend.native);
        expect(manager.mapTracking, isTrue);
        expect(platform.watchedLocations!.hasListener, isFalse);
        expect(nativePlatform.positions.hasListener, isTrue);
        platform.watchedLocations!.add(_traceletFix(9));
        nativePlatform.emit(3);
        await Future<void>.delayed(const Duration(milliseconds: 120));
        expect(manager.latestPosition?.longitude, 3);

        expect(
          await manager.setLocationBackend(LocationBackend.tracelet),
          isTrue,
        );
        expect(manager.mapTracking, isTrue);
        expect(selectedLocationBackend, LocationBackend.tracelet);
        expect(nativePlatform.positions.hasListener, isFalse);
        expect(nativePlatform.starts, 1);
        expect(nativePlatform.stops, 1);
        expect(platform.watchCalls, 2);
        expect(platform.stopWatchCalls, 1);
        nativePlatform.emit(9);
        platform.watchedLocations!.add(_traceletFix(4));
        await Future<void>.delayed(const Duration(milliseconds: 120));
        expect(manager.latestPosition?.longitude, 4);
        expect(backend.recorded, isEmpty);
      },
    );

    test('recording prevents switching providers', () async {
      manager.readyToStart();
      await manager.toggleMapTracking(false);
      expect(manager.recordingStatus, GpsRecordingStatus.recording);
      expect(await manager.setLocationBackend(LocationBackend.native), isFalse);
      expect(manager.locationBackend, LocationBackend.tracelet);
      expect(platform.enabled, isTrue);
      expect(nativePlatform.starts, 0);
    });

    for (final target in [LocationBackend.native, LocationBackend.tracelet]) {
      test(
        'a failed $target start restores following and permits retry',
        () async {
          await startFollowing();
          if (target == LocationBackend.tracelet) {
            await manager.setLocationBackend(LocationBackend.native);
            platform.failWatch = true;
          } else {
            nativePlatform.failStart = true;
          }
          final previous = manager.locationBackend;

          await expectLater(
            manager.setLocationBackend(target),
            throwsStateError,
          );
          expect(manager.locationBackend, previous);
          expect(selectedLocationBackend, previous);
          expect(manager.mapTracking, isTrue);
          expect(
            nativePlatform.positions.hasListener,
            previous == LocationBackend.native,
          );
          expect(
            platform.watchedLocations!.hasListener,
            previous == LocationBackend.tracelet,
          );

          nativePlatform.failStart = false;
          platform.failWatch = false;
          expect(await manager.setLocationBackend(target), isTrue);
          expect(manager.locationBackend, target);
          expect(selectedLocationBackend, target);
          expect(manager.mapTracking, isTrue);
        },
      );
    }

    test(
      'a failed preference write stops the target and restores following',
      () async {
        await startFollowing();
        preferences.failedKey = 'Settings.useLegacyLocationBackend';

        await expectLater(
          manager.setLocationBackend(LocationBackend.native),
          throwsStateError,
        );
        expect(manager.locationBackend, LocationBackend.tracelet);
        expect(selectedLocationBackend, LocationBackend.tracelet);
        expect(manager.mapTracking, isTrue);
        expect(nativePlatform.positions.hasListener, isFalse);
        expect(nativePlatform.starts, 1);
        expect(nativePlatform.stops, 1);
        expect(platform.watchedLocations!.hasListener, isTrue);

        preferences.failedKey = null;
        expect(
          await manager.setLocationBackend(LocationBackend.native),
          isTrue,
        );
      },
    );

    test(
      'a failed pause flush remains paused and is retried before finalizing',
      () async {
        manager.readyToStart();
        await manager.toggleMapTracking(false);
        expect(manager.recordingStatus, GpsRecordingStatus.recording);
        backend.events.clear();
        platform.rows.add(_first);
        backend.failWrites = true;

        await expectLater(
          manager.changeRecordingState(GpsRecordingStatus.paused),
          throwsStateError,
        );
        expect(manager.recordingStatus, GpsRecordingStatus.paused);
        expect(platform.enabled, isFalse);
        expect(preferences.values[MMKVKey.isRecording], isFalse);
        expect(preferences.values[_pendingQueueKey], isTrue);
        expect(backend.events, ['record']);

        backend.failWrites = false;
        await manager.changeRecordingState(GpsRecordingStatus.none);
        expect(backend.events, ['record', 'record', 'finalize']);
        expect(backend.recorded, hasLength(1));
        expect(platform.rows, isEmpty);
        expect(preferences.values[_pendingQueueKey], isFalse);
        expect(manager.recordingStatus, GpsRecordingStatus.none);
      },
    );

    test('a failed finish flush remains paused and records the queue before retrying finalization', () async {
      manager.readyToStart();
      await manager.toggleMapTracking(false);
      expect(manager.recordingStatus, GpsRecordingStatus.recording);
      backend.events.clear();
      platform.rows.add(_first);
      backend.failWrites = true;

      await expectLater(
        manager.changeRecordingState(GpsRecordingStatus.none),
        throwsStateError,
      );
      expect(manager.recordingStatus, GpsRecordingStatus.paused);
      expect(platform.enabled, isFalse);
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(preferences.values[_pendingQueueKey], isTrue);
      expect(platform.rows, [_first]);
      expect(backend.recorded, isEmpty);
      expect(backend.events, ['record']);

      backend.failWrites = false;
      await manager.changeRecordingState(GpsRecordingStatus.none);
      expect(backend.events, ['record', 'record', 'finalize']);
      expect(backend.recorded, hasLength(1));
      expect(platform.rows, isEmpty);
      expect(preferences.values[_pendingQueueKey], isFalse);
      expect(manager.recordingStatus, GpsRecordingStatus.none);
    });

    test('a failed native stop remains recording and can retry finishing without losing queued fixes', () async {
      manager.readyToStart();
      await manager.toggleMapTracking(false);
      expect(manager.recordingStatus, GpsRecordingStatus.recording);
      backend.events.clear();
      platform.operations.clear();
      platform.rows.add(_first);
      platform.failStop = true;

      await expectLater(
        manager.changeRecordingState(GpsRecordingStatus.none),
        throwsStateError,
      );
      platform.failStop = false;
      expect(manager.recordingStatus, GpsRecordingStatus.recording);
      expect(platform.enabled, isTrue);
      expect(preferences.values[_pendingQueueKey], isTrue);
      expect(platform.rows, [_first]);
      expect(backend.recorded, isEmpty);
      expect(backend.events, isEmpty);

      await manager.changeRecordingState(GpsRecordingStatus.none);
      expect(platform.operations, ['stop', 'stop']);
      expect(platform.enabled, isFalse);
      expect(backend.events, ['record', 'finalize']);
      expect(backend.recorded, hasLength(1));
      expect(platform.rows, isEmpty);
      expect(preferences.values[_pendingQueueKey], isFalse);
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(manager.recordingStatus, GpsRecordingStatus.none);
    });

    test('a failed cold start leaves recording paused', () async {
      platform.failStart = true;
      final paused = Completer<void>();
      manager.addListener(() {
        if (manager.recordingStatus == GpsRecordingStatus.paused &&
            !paused.isCompleted) {
          paused.complete();
        }
      });

      manager.readyToStart();
      await paused.future.timeout(const Duration(seconds: 2));
      await manager.toggleMapTracking(false);
      expect(manager.recordingStatus, GpsRecordingStatus.paused);
      expect(platform.enabled, isFalse);
      expect(preferences.values[MMKVKey.isRecording], isFalse);
      expect(
        platform.operations.where((operation) => operation == 'start'),
        hasLength(1),
      );
    });

    test(
      'live fixes update the map without recording the same point twice',
      () async {
        platform.liveLocations = StreamController<TlLocation>.broadcast();
        manager.readyToStart();
        await manager.toggleMapTracking(false);
        platform.rows.add(_first);
        platform.liveLocations!.add(_traceletFix(2));
        await Future<void>.delayed(Duration.zero);

        expect(manager.latestPosition?.latitude, 1);
        expect(backend.recorded, hasLength(1));
        expect(platform.rows, isEmpty);
        await manager.changeRecordingState(GpsRecordingStatus.paused);
      },
    );
  });

  test(
    'Tracelet acknowledges committed fixes and replays failed acknowledgements',
    () async {
      RustLib.initMock(api: _TraceletRustApi());
      addTearDown(RustLib.dispose);
      final platform = _TraceletQueue();
      final previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      addTearDown(() {
        Tracelet.removeListeners();
        TraceletPlatform.instance = previousPlatform;
      });
      preferences.values[_pendingQueueKey] = true;
      final recorded = <double>[];
      var failCommit = true;
      final service = TraceletLocationService((data) async {
        expect(
          data.timestampMs,
          DateTime.utc(2026, 11, 7).millisecondsSinceEpoch,
        );
        if (failCommit && data.longitude == 3) {
          throw StateError('Could not commit fix');
        }
        recorded.add(data.longitude);
      });
      await expectLater(service.flushRecording(), throwsStateError);
      expect(platform.rows, [_second, _third]);
      expect(recorded, [2.0]);

      failCommit = false;
      platform.failedAcknowledgement = _second;
      await expectLater(service.flushRecording(), throwsStateError);
      expect(platform.rows, [_second, _third]);
      expect(recorded, [2.0, 3.0]);

      platform.failedAcknowledgement = null;
      await service.flushRecording();
      expect(platform.rows, isEmpty);
      expect(recorded, [2.0, 3.0, 3.0, 4.0]);
      expect(preferences.values[_pendingQueueKey], isFalse);
    },
  );

  test('Checks a cache drain once before its first eligible fix', () async {
    RustLib.initMock(api: _TraceletRustApi());
    addTearDown(RustLib.dispose);
    final previousPlatform = TraceletPlatform.instance;
    final platform = _TraceletQueue();
    TraceletPlatform.instance = platform;
    addTearDown(() {
      Tracelet.removeListeners();
      TraceletPlatform.instance = previousPlatform;
    });
    preferences.values[_pendingQueueKey] = true;

    final operations = <String>[];
    final service = TraceletLocationService(
      (data) async => operations.add('record ${data.longitude}'),
      checkBatchStart: (data) async {
        operations.add('check ${data.longitude}');
        return data.longitude != 2;
      },
    );
    await service.flushRecording();

    expect(operations, [
      'check 2.0',
      'record 2.0',
      'check 3.0',
      'record 3.0',
      'record 4.0',
    ]);
    expect(platform.rows, isEmpty);
  });

  test(
    'Queued fixes retain capture time and record app receipt time',
    () async {
      RustLib.initMock(api: _TraceletRustApi());
      addTearDown(RustLib.dispose);
      final backend = _AppRecordingApi();
      app_rust.RustLib.initMock(api: backend);
      addTearDown(app_rust.RustLib.dispose);
      final platform = _TraceletQueue();
      final previousPlatform = TraceletPlatform.instance;
      TraceletPlatform.instance = platform;
      addTearDown(() {
        Tracelet.removeListeners();
        TraceletPlatform.instance = previousPlatform;
      });

      preferences.values[_pendingQueueKey] = true;
      final before = DateTime.now().millisecondsSinceEpoch;
      await TraceletLocationService(_recordLocation).flushRecording();
      final after = DateTime.now().millisecondsSinceEpoch;

      expect(backend.recorded, hasLength(3));
      for (final point in backend.recorded) {
        expect(
          point.rawGpsPoint.timestampMs,
          DateTime.utc(2026, 11, 7).millisecondsSinceEpoch,
        );
        expect(point.receivedTimestampMs, inInclusiveRange(before, after));
      }
      expect(platform.rows, isEmpty);
    },
  );
}

Future<void> _recordLocation(LocationData data) async {
  await recordLocation(data, receivedAt: DateTime.now());
}

TlLocation _traceletFix(double longitude) => TlLocation(
  coords: TlCoords(
    latitude: 1,
    longitude: longitude,
    accuracy: 5,
    speed: 1,
    heading: 0,
    altitude: 0,
    altitudeAccuracy: 0,
    speedAccuracy: 0,
    headingAccuracy: 0,
  ),
  battery: TlBattery(level: 1, isCharging: false),
  timestamp: DateTime.now().toUtc().toIso8601String(),
  uuid: _first,
  isMoving: true,
  odometer: 0,
);

class _NativeLocationPlatform extends GeolocatorPlatform {
  late final positions = StreamController<Position>.broadcast(
    onListen: () => starts++,
    onCancel: () => stops++,
  );
  int starts = 0;
  int stops = 0;
  bool failStart = false;

  @override
  Future<bool> isLocationServiceEnabled() async => true;

  @override
  Future<Position?> getLastKnownPosition({
    bool forceLocationManager = false,
  }) async => null;

  @override
  Stream<Position> getPositionStream({LocationSettings? locationSettings}) {
    if (failStart) throw StateError('Could not start native locations');
    return positions.stream;
  }

  void emit(double longitude) => positions.add(
    Position(
      longitude: longitude,
      latitude: 1,
      timestamp: DateTime.now(),
      accuracy: 5,
      altitude: 0,
      altitudeAccuracy: 0,
      heading: 0,
      headingAccuracy: 0,
      speed: 1,
      speedAccuracy: 0,
    ),
  );
}

class _AppRecordingApi extends Fake implements app_rust.RustLibApi {
  final recorded = <ExtendedRawGPSPoint>[];
  bool failWrites = false;
  bool hasOngoingJourney = false;
  final events = <String>[];

  @override
  Future<bool> crateApiApiHasOngoingJourney() async => hasOngoingJourney;

  @override
  Future<bool> crateApiApiTryAutoFinalizeJourney() async => false;

  @override
  Future<bool?> crateApiApiTryAutoFinalizeBeforeLocation({
    required ExtendedRawGPSPoint data,
  }) async => false;

  @override
  Future<bool> crateApiApiFinalizeOngoingJourney() async {
    events.add('finalize');
    final finalized = hasOngoingJourney;
    hasOngoingJourney = false;
    return finalized;
  }

  @override
  Future<bool> crateApiApiOnLocationUpdate({
    required ExtendedRawGPSPoint data,
  }) async {
    events.add('record');
    if (failWrites) throw StateError('Could not commit fix');
    hasOngoingJourney = true;
    recorded.add(data);
    return true;
  }
}

class _TraceletRustApi extends Fake implements RustLibApi {
  @override
  TripManagerDart crateApiDartTripTripManagerDartNew() => _TripManager();
}

class _TripManager extends Fake implements TripManagerDart {
  @override
  void reset() {}

  @override
  void onLocationReceived({
    required double latitude,
    required double longitude,
    required int timestampMs,
  }) {}
}

class _HeartbeatTracelet extends TraceletPlatform {
  @override
  Future<List<Map<String, Object?>>> getLocations([
    Map<String, Object?>? query,
  ]) async => [];

  TlConfig? config;
  bool? motionActivityDisabledOnReady;
  int motionRequests = 0;
  bool enabled = false;
  bool failStart = false;
  bool failStop = false;
  bool failWatch = false;
  int watchCalls = 0;
  int stopWatchCalls = 0;
  final operations = <String>[];
  StreamController<TlLocation>? liveLocations;
  StreamController<TlLocation>? watchedLocations;

  @override
  Future<MotionAuthorizationStatus> requestMotionAuthorization() async {
    motionRequests++;
    return MotionAuthorizationStatus.granted;
  }

  @override
  Future<Map<String, Object?>> setConfig(TlConfig config) async {
    this.config!.motion.disableMotionActivityUpdates =
        config.motion.disableMotionActivityUpdates;
    return {'enabled': false};
  }

  @override
  Stream<Map<String?, Object?>> get remoteConfigEvents => const Stream.empty();
  @override
  Stream<TlLocation> get locationEvents =>
      liveLocations?.stream ?? const Stream.empty();
  @override
  Stream<TlLocation> get watchPositionEvents =>
      watchedLocations?.stream ?? const Stream.empty();
  @override
  Stream<TlLocation> get motionChangeEvents => const Stream.empty();
  @override
  Future<Map<String, Object?>> getState() async => {'enabled': enabled};
  @override
  Future<Map<String, Object?>> ready(TlConfig config) async {
    operations.add('ready');
    this.config = config;
    motionActivityDisabledOnReady = config.motion.disableMotionActivityUpdates;
    return {'enabled': false};
  }

  @override
  Future<Map<String, Object?>> start() async {
    operations.add('start');
    if (failStart) throw StateError('Could not start location tracking');
    enabled = true;
    return {'enabled': true};
  }

  @override
  Future<Map<String, Object?>> stop() async {
    operations.add('stop');
    if (failStop) throw StateError('Could not stop location tracking');
    enabled = false;
    return {'enabled': false};
  }

  @override
  Future<int> watchPosition(TlCurrentPositionOptions options) async {
    watchCalls++;
    if (failWatch) throw StateError('Could not watch positions');
    return watchCalls;
  }

  @override
  Future<bool> stopWatchPosition(int watchId) async {
    stopWatchCalls++;
    return watchId > 0;
  }
}

class _TraceletQueue extends _HeartbeatTracelet {
  final rows = [_first, _invalid, _second, _third];
  String? failedAcknowledgement;
  bool _ready = false;

  @override
  Stream<Map<String?, Object?>> get remoteConfigEvents => const Stream.empty();

  @override
  Future<Map<String, Object?>> ready(TlConfig config) async {
    _ready = true;
    return super.ready(config);
  }

  @override
  Future<List<Map<String, Object?>>> getLocations([
    Map<String, Object?>? query,
  ]) async {
    expect(_ready, isTrue);
    return [
      for (final uuid in rows)
        {
          'uuid': uuid,
          'timestamp': '2026-11-07T00:00:00Z',
          'coords': {
            'latitude': uuid == _invalid ? 91 : 1,
            'longitude': switch (uuid) {
              _second => 3,
              _third => 4,
              _ => 2,
            },
            'accuracy': 5,
          },
        },
    ];
  }

  @override
  Future<bool> destroyLocation(String uuid) async =>
      uuid != failedAcknowledgement && rows.remove(uuid);
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
