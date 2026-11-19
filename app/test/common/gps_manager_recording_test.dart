import 'dart:async';
import 'dart:ffi';

// ignore: depend_on_referenced_packages
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show AppLifecycleState;
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/mmkv_util.dart';
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
  var failCancelNotification = false;

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
      (call) async {
        if (failCancelNotification &&
            call.method == 'cancelNotificationOnKillService') {
          throw PlatformException(code: 'injected_cancel_failure');
        }
        return true;
      },
    );
    messenger.setMockMethodCallHandler(
      const MethodChannel('PonnamKarthik/fluttertoast'),
      (_) async => true,
    );
    await MMKVUtil.init();
    RustLib.initMock(api: backend);
  });

  setUp(() {
    failCancelNotification = false;
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
    locations.emitFix();
    await received.future;
    failCancelNotification = true;
    await expectLater(
      gps.changeRecordingState(GpsRecordingStatus.none),
      throwsStateError,
    );
    expect(locations.positions.hasListener, isFalse);
    expect(gps.recordingStatus, GpsRecordingStatus.paused);
    expect(MMKVUtil.getBool(MMKVKey.isRecording), isFalse);
    expect(await gps.toggleMapTracking(true), isTrue);
    expect(locations.positions.hasListener, isTrue);

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
    'pause drains the last fix and resume checks before acquisition',
    () async {
      final gps = GpsManager();
      addTearDown(gps.dispose);
      gps.readyToStart();
      await gps.changeRecordingState(GpsRecordingStatus.recording);

      final writing = Completer<void>();
      final releaseWrite = Completer<void>();
      backend.write = (points) {
        expect(points, hasLength(1));
        writing.complete();
        return releaseWrite.future;
      };
      locations.emitFix();
      final pause = gps.changeRecordingState(GpsRecordingStatus.paused);
      await writing.future;
      expect(gps.recordingStatus, GpsRecordingStatus.recording);
      releaseWrite.complete();
      await pause;
      expect(backend.pausedChecks, 0);
      expect(locations.positions.hasListener, isFalse);

      gps.didChangeAppLifecycleState(AppLifecycleState.resumed);
      await gps.toggleMapTracking(false);
      expect(backend.pausedChecks, 1);

      final checking = Completer<void>();
      final expired = Completer<bool>();
      backend.finalizePaused = () {
        checking.complete();
        return expired.future;
      };
      final finalized = gps.journeyFinalized.first;
      final resume = gps.changeRecordingState(GpsRecordingStatus.recording);
      await checking.future;
      expect(locations.positions.hasListener, isFalse);
      gps.didChangeAppLifecycleState(AppLifecycleState.resumed);
      expired.complete(true);
      await resume;
      await finalized;
      await gps.toggleMapTracking(false);
      expect(locations.positions.hasListener, isTrue);
      expect(backend.pausedChecks, 2);
      expect(gps.recordingStatus, GpsRecordingStatus.recording);
    },
  );

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
      final finalized = gps.journeyFinalized.first;
      gps.readyToStart();
      await finalized;
      expect(gps.recordingStatus, GpsRecordingStatus.none);
      expect(backend.pausedChecks, 1);
      expect(MMKVUtil.getBool(MMKVKey.isRecording), isFalse);
    },
  );

  test(
    'recording reorders a tail fix arriving after the next journey fix',
    () async {
      final gps = GpsManager();
      addTearDown(gps.dispose);
      gps.readyToStart();
      await gps.changeRecordingState(GpsRecordingStatus.recording);

      final received = <int>[];
      final written = Completer<void>();
      backend.write = (points) async {
        received.addAll(points.map((point) => point.rawGpsPoint.timestampMs!));
        if (received.length == 2) written.complete();
      };
      final newer = DateTime.now();
      final tail = newer.subtract(const Duration(hours: 7));
      locations.emitFix(timestamp: newer);
      await Future<void>.delayed(const Duration(milliseconds: 150));
      locations.emitFix(timestamp: tail);
      await written.future;
      expect(received, [
        tail.millisecondsSinceEpoch,
        newer.millisecondsSinceEpoch,
      ]);
    },
  );

  test('dispose lets an accepted end save before clearing recording', () async {
    final gps = GpsManager();
    gps.readyToStart();
    await gps.changeRecordingState(GpsRecordingStatus.recording);

    final writing = Completer<void>();
    final release = Completer<void>();
    backend.write = (_) {
      writing.complete();
      return release.future;
    };
    var finalized = false;
    backend.finalize = () async => finalized = true;
    locations.emitFix();
    final end = gps.changeRecordingState(GpsRecordingStatus.none);
    await writing.future;
    gps.dispose();
    release.complete();
    await end;
    expect(finalized, isTrue);
    expect(locations.positions.hasListener, isFalse);
    expect(MMKVUtil.getBool(MMKVKey.isRecording), isFalse);
  });
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

  void emitFix({DateTime? timestamp}) => positions.add(
    Position(
      latitude: 30,
      longitude: 120,
      timestamp: timestamp ?? DateTime.now(),
      accuracy: 5,
      altitude: 0,
      altitudeAccuracy: 1,
      heading: 0,
      headingAccuracy: 1,
      speed: 0,
      speedAccuracy: 1,
    ),
  );

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
  Future<bool> crateApiApiOnLocationUpdates({
    required List<ExtendedRawGPSPoint> updates,
  }) async {
    await write(updates);
    return false;
  }
}
