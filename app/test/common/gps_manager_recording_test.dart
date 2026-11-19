import 'dart:async';
import 'dart:ffi';

// ignore: depend_on_referenced_packages
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/gps.dart';
// ignore: depend_on_referenced_packages
import 'package:mmkv_platform_interface/mmkv_platform_interface.dart';
import 'package:permission_handler/permission_handler.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final settings = _MemorySettings();
  final locations = _LocationSource();
  final backend = _RecordingApi();

  setUpAll(() async {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    MMKVPluginPlatform.instance = settings;
    GeolocatorPlatform.instance = locations;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(
      const MethodChannel('flutter.baseflow.com/permissions/methods'),
      (call) async => call.arguments == Permission.notification.value
          ? PermissionStatus.denied.index
          : PermissionStatus.granted.index,
    );
    messenger.setMockMethodCallHandler(
      const MethodChannel('notification_when_app_is_killed'),
      (_) async => true,
    );
    messenger.setMockMethodCallHandler(
      const MethodChannel('PonnamKarthik/fluttertoast'),
      (_) async => true,
    );
    await MMKVUtil.init();
    RustLib.initMock(api: backend);
  });

  setUp(() {
    settings.values.clear();
    backend.hasOngoing = false;
    backend.pausedChecks = 0;
    backend.write = (_) async {};
    backend.finalize = () async => true;
    backend.finalizePaused = () async => false;
  });

  tearDownAll(() async {
    await locations.positions.close();
    debugDefaultTargetPlatformOverride = null;
  });

  test('ending can retry a failed drain and still save the journey', () async {
    final gps = GpsManager();
    addTearDown(gps.dispose);
    gps.readyToStart();
    await gps.changeRecordingState(GpsRecordingStatus.recording);

    final received = Completer<void>();
    backend.write = (_) async {
      if (!received.isCompleted) received.complete();
      throw StateError('injected write failure');
    };
    locations.positions.add(
      Position(
        latitude: 30,
        longitude: 120,
        timestamp: DateTime.now(),
        accuracy: 5,
        altitude: 0,
        altitudeAccuracy: 1,
        heading: 0,
        headingAccuracy: 1,
        speed: 0,
        speedAccuracy: 1,
      ),
    );
    await received.future;
    await expectLater(
      gps.changeRecordingState(GpsRecordingStatus.none),
      throwsStateError,
    );
    expect(gps.recordingStatus, GpsRecordingStatus.paused);
    expect(MMKVUtil.getBool(MMKVKey.isRecording), isFalse);

    var savedPoints = 0;
    backend.write = (points) async => savedPoints += points.length;
    var finalized = 0;
    backend.finalize = () async {
      expect(savedPoints, 1);
      finalized++;
      return true;
    };
    await gps.changeRecordingState(GpsRecordingStatus.none);
    expect(finalized, 1);
    expect(gps.recordingStatus, GpsRecordingStatus.none);
  });

  test('ending keeps the paused state until finalization commits', () async {
    backend.hasOngoing = true;
    final gps = GpsManager();
    addTearDown(gps.dispose);
    gps.readyToStart();

    final saving = Completer<void>();
    final result = Completer<bool>();
    backend.finalize = () {
      saving.complete();
      return result.future;
    };
    final end = gps.changeRecordingState(GpsRecordingStatus.none);
    await saving.future;
    expect(gps.recordingStatus, GpsRecordingStatus.paused);
    final failed = expectLater(end, throwsStateError);
    result.completeError(StateError('injected finalization failure'));
    await failed;
    expect(gps.recordingStatus, GpsRecordingStatus.paused);

    backend.finalize = () async => true;
    await gps.changeRecordingState(GpsRecordingStatus.none);
    expect(gps.recordingStatus, GpsRecordingStatus.none);
  });

  test(
    'pausing before acquisition starts still enables paused checks',
    () async {
      MMKVUtil.putBool(MMKVKey.isRecording, true);
      final gps = GpsManager();
      addTearDown(gps.dispose);
      await gps.changeRecordingState(GpsRecordingStatus.paused);
      expect(gps.recordingStatus, GpsRecordingStatus.paused);
      expect(backend.pausedChecks, 0);

      backend.finalizePaused = () async => true;
      final finalized = Completer<void>();
      gps.addListener(() {
        if (gps.recordingStatus == GpsRecordingStatus.none &&
            !finalized.isCompleted) {
          finalized.complete();
        }
      });
      gps.readyToStart();
      await finalized.future;
      expect(backend.pausedChecks, 1);
      expect(MMKVUtil.getBool(MMKVKey.isRecording), isFalse);
    },
  );
}

class _MemorySettings extends MMKVPluginPlatform {
  final values = <String, int>{};

  @override
  Future<String> getApplicationDocumentsPath() async => '/tmp';

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
        values[key.toDartString()] = value;
        return 1;
      };

  @override
  int Function(Pointer<Void>, Pointer<Utf8>, int) decodeBoolFunc() =>
      (_, key, fallback) => values[key.toDartString()] ?? fallback;
}

class _LocationSource extends GeolocatorPlatform {
  final positions = StreamController<Position>.broadcast();

  @override
  Future<bool> isLocationServiceEnabled() async => true;

  @override
  Future<Position?> getLastKnownPosition({
    bool forceLocationManager = false,
  }) async => null;

  @override
  Stream<Position> getPositionStream({LocationSettings? locationSettings}) =>
      positions.stream;
}

class _RecordingApi extends Fake implements RustLibApi {
  bool hasOngoing = false;
  int pausedChecks = 0;
  late Future<void> Function(List<ExtendedRawGPSPoint>) write;
  late Future<bool> Function() finalize;
  late Future<bool> Function() finalizePaused;

  @override
  Future<bool> crateApiApiHasOngoingJourney() async => hasOngoing;

  @override
  Future<bool> crateApiApiTryFinalizePausedJourney() {
    pausedChecks++;
    return finalizePaused();
  }

  @override
  Future<bool> crateApiApiFinalizeOngoingJourney() => finalize();

  @override
  Future<api.RecordingResult> crateApiApiOnLocationUpdates({
    required List<ExtendedRawGPSPoint> updates,
  }) async {
    await write(updates);
    return const api.RecordingResult(meaningful: true, journeysFinalized: 0);
  }
}
