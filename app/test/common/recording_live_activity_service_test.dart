import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/recording_live_activity_service.dart';
import 'package:memolanes/common/service/location/location_service.dart';

class _Gps extends ChangeNotifier implements GpsManager {
  _Gps([this.recordingStatus = GpsRecordingStatus.none])
    : _recordingStream = recordingStatus == GpsRecordingStatus.recording;

  @override
  GpsRecordingStatus recordingStatus;
  bool _recordingStream;
  LocationData? _latestPosition;
  @override
  Future<void> initialized = Future<void>.value();
  @override
  LocationData? get recordingPosition =>
      _recordingStream ? _latestPosition : null;

  void status(GpsRecordingStatus status) {
    final changed = recordingStatus != status;
    recordingStatus = status;
    // Main notifies before switching the stream, then clears the old stream's
    // fix and notifies again. Map-only fixes aren't recordingPosition.
    notifyListeners();
    if (changed) {
      _latestPosition = null;
      _recordingStream = status == GpsRecordingStatus.recording;
      notifyListeners();
    }
  }

  void expirePosition() {
    _latestPosition = null;
    notifyListeners();
  }

  void position(
    DateTime timestamp, {
    double accuracy = 8,
    double latitude = 30,
  }) {
    _latestPosition = LocationData(
      latitude: latitude,
      longitude: 120,
      accuracy: accuracy,
      timestampMs: timestamp.millisecondsSinceEpoch,
    );
    notifyListeners();
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  testWidgets('startup waits for restored business state before clearing', (
    tester,
  ) async {
    final restored = Completer<void>();
    final gps = _Gps()..initialized = restored.future;
    final payloads = <Map<String, Object?>>[];
    final service = RecordingLiveActivityService(
      gps,
      synchronize: (p) async => payloads.add(p),
    );
    final ready = service.initialize();
    await tester.pump();
    expect(payloads, isEmpty);
    gps.recordingStatus = GpsRecordingStatus.paused;
    restored.complete();
    await ready;
    await service.idle;
    expect(payloads.single['status'], 'paused');
    service.dispose();
    gps.dispose();
  });

  testWidgets('fresh recording fix from before service startup stays usable', (
    tester,
  ) async {
    final now = DateTime(2026, 9, 29);
    final gps = _Gps(GpsRecordingStatus.recording);
    gps.position(now.subtract(const Duration(seconds: 2)), accuracy: 7);
    final payloads = <Map<String, Object?>>[];
    final service = RecordingLiveActivityService(
      gps,
      now: () => now,
      synchronize: (p) async => payloads.add(p),
    );
    await service.initialize();
    await service.idle;
    expect(payloads.single['accuracy'], 7);
    expect(
      payloads.single['gpsTimestampMs'],
      now.subtract(const Duration(seconds: 2)).millisecondsSinceEpoch,
    );
    service.dispose();
    gps.dispose();
  });

  testWidgets('failed startup cannot send a false stop through flush', (
    tester,
  ) async {
    final gps = _Gps()
      ..initialized = Future<void>.error(StateError('startup failed'));
    final payloads = <Map<String, Object?>>[];
    final service = RecordingLiveActivityService(
      gps,
      synchronize: (p) async => payloads.add(p),
    );
    await service.initialize();
    await service.flushPending();
    service.didChangeAppLifecycleState(AppLifecycleState.resumed);
    await service.idle;
    expect(payloads, isEmpty);
    service.dispose();
    gps.dispose();
  });

  testWidgets('in-flight creation cannot erase pause and stop transitions', (
    tester,
  ) async {
    final gps = _Gps(GpsRecordingStatus.recording);
    final inFlight = Completer<void>();
    final payloads = <Map<String, Object?>>[];
    final service = RecordingLiveActivityService(
      gps,
      synchronize: (p) async {
        payloads.add(p);
        if (payloads.length == 1) await inFlight.future;
      },
    );
    await service.initialize();
    await tester.pump();
    gps.status(GpsRecordingStatus.paused);
    gps.status(GpsRecordingStatus.none);
    inFlight.complete();
    await service.idle;
    expect(payloads.map((p) => p['status']), ['recording', 'paused', 'none']);
    expect(payloads.last['accuracy'], isNull);
    await tester.pump(const Duration(seconds: 6));
    expect(payloads, hasLength(3));
    service.dispose();
    gps.dispose();
  });

  testWidgets(
    'stop then immediate start preserves the native session boundary',
    (tester) async {
      final gps = _Gps(GpsRecordingStatus.recording);
      final payloads = <Map<String, Object?>>[];
      final service = RecordingLiveActivityService(
        gps,
        synchronize: (p) async => payloads.add(p),
      );
      await service.initialize();
      await service.idle;
      gps.status(GpsRecordingStatus.none);
      gps.status(GpsRecordingStatus.recording);
      await service.idle;
      expect(payloads.map((p) => p['status']), [
        'recording',
        'none',
        'recording',
      ]);
      service.dispose();
      gps.dispose();
    },
  );

  testWidgets(
    'main expiry notification bypasses throttle, latest update survives tail',
    (tester) async {
      var now = DateTime(2026, 9, 29);
      final gps = _Gps(GpsRecordingStatus.recording);
      final payloads = <Map<String, Object?>>[];
      final service = RecordingLiveActivityService(
        gps,
        now: () => now,
        synchronize: (p) async => payloads.add(p),
      );
      await service.initialize();
      await service.idle;
      gps.position(now);
      await service.idle;
      expect(payloads.last['accuracy'], 8);
      now = now.add(const Duration(seconds: 1));
      await tester.pump(const Duration(seconds: 1));
      gps.position(now, accuracy: 9);
      now = now.add(const Duration(seconds: 1));
      await tester.pump(const Duration(seconds: 1));
      gps.position(now, accuracy: 10);
      expect(payloads.last['accuracy'], 8);
      now = now.add(const Duration(seconds: 3));
      await tester.pump(const Duration(seconds: 3));
      await service.idle;
      expect(payloads.last['accuracy'], 10);
      expect(payloads.last.keys, isNot(contains('latitude')));
      expect(payloads.last.keys, isNot(contains('longitude')));
      // A fix remains valid at 11 seconds. Send it, then simulate main's
      // 12-second clearing notification within the ordinary 5-second throttle.
      now = now.add(const Duration(seconds: 8));
      await tester.pump(const Duration(seconds: 8));
      gps.position(now.subtract(const Duration(seconds: 11)), accuracy: 11);
      await service.idle;
      expect(payloads.last['accuracy'], 11);
      now = now.add(const Duration(seconds: 1));
      await tester.pump(const Duration(seconds: 1));
      gps.expirePosition();
      await service.idle;
      expect(payloads.last['gpsTimestampMs'], isNull);
      expect(payloads.last['accuracy'], isNull);
      service.dispose();
      gps.dispose();
    },
  );

  testWidgets(
    'recording stream excludes map fixes, invalid precision and future fixes',
    (tester) async {
      var now = DateTime(2026, 9, 29);
      final gps = _Gps(GpsRecordingStatus.recording);
      final payloads = <Map<String, Object?>>[];
      final service = RecordingLiveActivityService(
        gps,
        now: () => now,
        synchronize: (p) async => payloads.add(p),
      );
      await service.initialize();
      await service.idle;
      gps.position(now);
      await service.idle;
      gps.status(GpsRecordingStatus.paused);
      await service.idle;
      now = now.add(const Duration(seconds: 1));
      gps.position(now); // Map-only position while paused.
      gps.status(GpsRecordingStatus.recording);
      await service.idle;
      expect(payloads.last['accuracy'], isNull);
      gps.position(now.add(const Duration(seconds: 1)));
      service.didChangeAppLifecycleState(AppLifecycleState.resumed);
      await service.idle;
      expect(payloads.last['accuracy'], isNull);
      gps.position(now, accuracy: double.nan);
      service.didChangeAppLifecycleState(AppLifecycleState.resumed);
      await service.idle;
      expect(payloads.last['accuracy'], isNull);
      gps.position(now, accuracy: 5);
      await service.idle;
      expect(payloads.last['accuracy'], 5);
      service.dispose();
      gps.dispose();
    },
  );

  testWidgets('unchanged foreground state still reconciles actual activities', (
    tester,
  ) async {
    final gps = _Gps()..recordingStatus = GpsRecordingStatus.paused;
    final payloads = <Map<String, Object?>>[];
    final service = RecordingLiveActivityService(
      gps,
      synchronize: (p) async => payloads.add(p),
    );
    await service.initialize();
    await service.idle;
    service.didChangeAppLifecycleState(AppLifecycleState.inactive);
    await service.idle;
    expect(payloads, hasLength(1));
    service.didChangeAppLifecycleState(AppLifecycleState.resumed);
    await service.idle;
    expect(payloads, hasLength(2));
    expect(payloads.first, payloads.last);
    service.dispose();
    gps.dispose();
  });

  testWidgets('queued success cannot clear a later failed cleanup', (
    tester,
  ) async {
    final first = Completer<void>();
    var calls = 0;
    final gps = _Gps();
    final service = RecordingLiveActivityService(
      gps,
      synchronize: (p) async {
        calls++;
        if (calls == 1) await first.future;
        if (calls == 2) throw StateError('injected channel failure');
      },
    );
    await service.initialize();
    await tester.pump();
    service.didChangeAppLifecycleState(AppLifecycleState.resumed);
    first.complete();
    await service.idle;
    expect(calls, 2);
    await tester.pump(const Duration(seconds: 30));
    await service.idle;
    expect(calls, 3);
    service.dispose();
    gps.dispose();
  });

  testWidgets(
    'failed synchronization retries on heartbeat without busy retries',
    (tester) async {
      var calls = 0;
      final gps = _Gps();
      final service = RecordingLiveActivityService(
        gps,
        synchronize: (p) async {
          calls++;
          if (calls == 1) throw StateError('injected channel failure');
        },
      );
      await service.initialize();
      await tester.pump();
      expect(calls, 1);
      await tester.pump(const Duration(seconds: 1));
      expect(calls, 1);
      expect(gps.recordingStatus, GpsRecordingStatus.none);
      await tester.pump(const Duration(seconds: 29));
      await service.idle;
      expect(calls, 2);
      await tester.pump(const Duration(seconds: 30));
      expect(calls, 2);
      service.dispose();
      gps.dispose();
    },
  );
}
