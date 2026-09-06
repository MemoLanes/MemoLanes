import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/service/location/location_service.dart';

class _FakeLocationService implements ILocationService {
  final controller = StreamController<LocationData>.broadcast();
  final events = <String>[];
  Future<void> Function(LocationBatchConsumer consumer)? recoverBehavior;
  Future<void> Function(LocationStartOptions options)? startBehavior;
  Future<void> Function()? stopBehavior;
  var stopCalls = 0;

  @override
  Stream<LocationData> get locations => controller.stream;

  @override
  LocationProviderInfo get providerInfo =>
      const LocationProviderInfo(id: 'test', displayName: _displayName);

  static String _displayName(BuildContext context) => 'test';

  @override
  Future<void> recoverPendingDeliveries(LocationBatchConsumer consumer) async {
    events.add('recover');
    await recoverBehavior?.call(consumer);
    events.add('recovered');
  }

  @override
  Future<void> setForeground(bool foreground) async {
    events.add(foreground ? 'foreground' : 'background');
  }

  @override
  Future<void> start(LocationStartOptions options) async {
    events.add('start');
    await startBehavior?.call(options);
    events.add('started');
  }

  @override
  Future<void> stop() async {
    stopCalls += 1;
    events.add('stop');
    await stopBehavior?.call();
    events.add('stopped');
  }

  @override
  Future<void> dispose() async {
    await stop();
    await controller.close();
  }
}

GpsManager _manager(
  _FakeLocationService provider, {
  GpsRecordingStatus initialState = GpsRecordingStatus.none,
  Future<bool> Function()? tryAutoFinalize,
  List<bool>? persistedStates,
}) {
  return GpsManager(
    locationServiceFactory: () => provider,
    initialStateLoader: () async => initialState,
    permissionRequester: () async => true,
    persistRecordingState: (recording) async {
      persistedStates?.add(recording);
    },
    configureKillNotification: (_) async {},
    showToast: (_) {},
    recordingHealthUpdater: (_) {},
    recordingWriter: (_) async => true,
    tryAutoFinalize: tryAutoFinalize ?? () async => false,
    finalizeJourney: () async => false,
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'ready waits for durable recovery before finalize and live start',
    () async {
      final provider = _FakeLocationService();
      final releaseRecovery = Completer<void>();
      provider.recoverBehavior = (_) => releaseRecovery.future;
      final manager = _manager(
        provider,
        initialState: GpsRecordingStatus.recording,
        tryAutoFinalize: () async {
          provider.events.add('finalize');
          return false;
        },
      );

      final ready = manager.readyToStart();
      await Future<void>.delayed(Duration.zero);
      expect(provider.events, ['recover']);

      releaseRecovery.complete();
      await ready;
      expect(provider.events, [
        'recover',
        'recovered',
        'finalize',
        'start',
        'started',
      ]);
      manager.dispose();
    },
  );

  test(
    'recovery failure is surfaced and leaves a persisted safe-off state',
    () async {
      final provider = _FakeLocationService();
      provider.recoverBehavior = (_) async =>
          throw StateError('recovery failed');
      final persistedStates = <bool>[];
      final manager = _manager(
        provider,
        initialState: GpsRecordingStatus.recording,
        persistedStates: persistedStates,
      );

      await expectLater(manager.readyToStart(), throwsStateError);
      expect(manager.recordingStatus, GpsRecordingStatus.none);
      expect(manager.mapTracking, isFalse);
      expect(persistedStates.last, isFalse);
      expect(provider.stopCalls, greaterThanOrEqualTo(1));
      manager.dispose();
    },
  );

  test(
    'partial start failure is stopped and rolled back to safe-off',
    () async {
      final provider = _FakeLocationService();
      provider.startBehavior = (_) async => throw StateError('start failed');
      final persistedStates = <bool>[];
      final manager = _manager(provider, persistedStates: persistedStates);
      await manager.readyToStart();

      await expectLater(
        manager.changeRecordingState(GpsRecordingStatus.recording),
        throwsStateError,
      );
      expect(provider.stopCalls, greaterThanOrEqualTo(1));
      expect(manager.recordingStatus, GpsRecordingStatus.none);
      expect(manager.mapTracking, isFalse);
      expect(persistedStates.last, isFalse);
      manager.dispose();
    },
  );

  test(
    'stop failure still runs cleanup and commits deterministic safe-off',
    () async {
      final provider = _FakeLocationService();
      final persistedStates = <bool>[];
      final manager = _manager(provider, persistedStates: persistedStates);
      await manager.readyToStart();
      await manager.changeRecordingState(GpsRecordingStatus.recording);
      provider.stopBehavior = () async => throw StateError('stop failed');

      await expectLater(
        manager.changeRecordingState(GpsRecordingStatus.paused),
        throwsStateError,
      );
      expect(manager.recordingStatus, GpsRecordingStatus.none);
      expect(manager.mapTracking, isFalse);
      expect(persistedStates.last, isFalse);
      manager.dispose();
    },
  );
}
