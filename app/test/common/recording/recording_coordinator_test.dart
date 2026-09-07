import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/recording/recording_coordinator.dart';
import 'package:memolanes/common/service/location/location_service.dart';

LocationData _location(int timestampMs, {double latitude = 1}) => LocationData(
  latitude: latitude,
  longitude: 2,
  accuracy: 1,
  timestampMs: timestampMs,
);

void main() {
  test('stably sorts a batch at the Rust-facing boundary', () async {
    final writes = <List<RecordingLocationUpdate>>[];
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (updates) async {
        writes.add(updates);
        return true;
      },
    );

    await coordinator.start();
    await coordinator.persistLocations([
      _location(30),
      _location(10),
      _location(20, latitude: 2),
      _location(20, latitude: 3),
    ], isReplay: true);

    expect(writes.single.map((update) => update.location.timestampMs), [
      10,
      20,
      20,
      30,
    ]);
    expect(writes.single[1].location.latitude, 2);
    expect(writes.single[2].location.latitude, 3);
    await coordinator.dispose();
  });

  test('deduplicates live and replay deliveries', () async {
    final written = <int>[];
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (updates) async {
        written.addAll(updates.map((update) => update.location.timestampMs));
        return true;
      },
    );
    final duplicate = _location(20);

    await coordinator.start();
    await coordinator.persistLocations([
      _location(10),
      duplicate,
    ], isReplay: false);
    await coordinator.persistLocations([
      duplicate,
      _location(30),
    ], isReplay: true);

    expect(written, [10, 20, 30]);
    await coordinator.dispose();
  });

  test('a false meaningful result still acknowledges the batch', () async {
    var writes = 0;
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (_) async {
        writes += 1;
        return false;
      },
    );
    final location = _location(10);

    await coordinator.start();
    expect(
      await coordinator.persistLocations([location], isReplay: true),
      isFalse,
    );
    await coordinator.persistLocations([location], isReplay: true);

    expect(writes, 1);
    await coordinator.dispose();
  });

  test('a failed batch remains retryable', () async {
    var writes = 0;
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (_) async {
        writes += 1;
        if (writes == 1) throw StateError('temporary failure');
        return true;
      },
    );
    final location = _location(10);

    await coordinator.start();
    await expectLater(
      coordinator.persistLocations([location], isReplay: true),
      throwsStateError,
    );
    await coordinator.persistLocations([location], isReplay: true);

    expect(writes, 2);
    await coordinator.dispose();
  });

  test(
    'serializes concurrent batches and stop waits for an in-flight write',
    () async {
      final firstStarted = Completer<void>();
      final releaseFirst = Completer<void>();
      final written = <int>[];
      final coordinator = RecordingCoordinator(
        onLocationUpdates: (updates) async {
          firstStarted.complete();
          await releaseFirst.future;
          written.add(updates.single.location.timestampMs);
          return true;
        },
      );

      await coordinator.start();
      final write = coordinator.persistLocations([
        _location(10),
      ], isReplay: false);
      await firstStarted.future;
      var stopped = false;
      final stop = coordinator.stop().then((_) => stopped = true);
      await Future<void>.delayed(Duration.zero);

      expect(stopped, isFalse);
      releaseFirst.complete();
      await Future.wait([write, stop]);
      expect(written, [10]);
      expect(stopped, isTrue);
    },
  );

  test('replay does not drive the live inactivity countdown', () async {
    var finalizeCalls = 0;
    var now = DateTime.fromMillisecondsSinceEpoch(0);
    final coordinator = RecordingCoordinator(
      now: () => now,
      tryAutoFinalize: () async {
        finalizeCalls += 1;
        return false;
      },
      onLocationUpdates: (_) async => true,
    );

    await coordinator.start();
    await coordinator.tryAutoFinalize();
    now = now.add(const Duration(minutes: 2));
    await coordinator.persistLocations([_location(10)], isReplay: true);
    expect(finalizeCalls, 1);

    await coordinator.persistLocations([_location(20)], isReplay: false);
    expect(finalizeCalls, 2);
    await coordinator.dispose();
  });

  test('serializes explicit finalization behind an in-flight write', () async {
    final writeStarted = Completer<void>();
    final releaseWrite = Completer<void>();
    var finalizeCalls = 0;
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (_) async {
        writeStarted.complete();
        await releaseWrite.future;
        return true;
      },
      tryAutoFinalize: () async {
        finalizeCalls += 1;
        return false;
      },
    );

    await coordinator.start();
    final write = coordinator.persistLocations([
      _location(10),
    ], isReplay: false);
    await writeStarted.future;
    final finalize = coordinator.tryAutoFinalize();
    await Future<void>.delayed(Duration.zero);
    expect(finalizeCalls, 0);

    releaseWrite.complete();
    await Future.wait([write, finalize]);
    expect(finalizeCalls, 1);
    await coordinator.dispose();
  });

  test('finalization callback may stop without re-entering the lock', () async {
    late RecordingCoordinator coordinator;
    final callbackCompleted = Completer<void>();
    coordinator = RecordingCoordinator(
      onLocationUpdates: (_) async => true,
      tryAutoFinalize: () async => true,
      onJourneyFinalized: () async {
        await coordinator.stop();
        callbackCompleted.complete();
      },
    );

    await coordinator.start();
    await coordinator.tryAutoFinalize().timeout(const Duration(seconds: 1));
    await callbackCompleted.future.timeout(const Duration(seconds: 1));
    await expectLater(
      coordinator.persistLocations([_location(10)], isReplay: false),
      throwsStateError,
    );
  });

  test('finalization notification does not hold the delivery future', () async {
    final callbackStarted = Completer<void>();
    final releaseCallback = Completer<void>();
    final coordinator = RecordingCoordinator(
      tryAutoFinalize: () async => true,
      onJourneyFinalized: () async {
        callbackStarted.complete();
        await releaseCallback.future;
      },
    );

    await coordinator.start();
    await coordinator.tryAutoFinalize().timeout(const Duration(seconds: 1));
    await callbackStarted.future;

    releaseCallback.complete();
    await coordinator.dispose();
  });

  test('recovers pending data before startup finalization', () async {
    final events = <String>[];
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (updates) async {
        events.add('write:${updates.single.location.timestampMs}');
        return true;
      },
      tryAutoFinalize: () async {
        events.add('finalize');
        return false;
      },
    );

    final meaningful = await coordinator.recoverPending((consumer) async {
      events.add('recover');
      await consumer([_location(10)], isReplay: true);
      events.add('recovered');
    }, remainActive: false);

    expect(events, ['recover', 'write:10', 'recovered', 'finalize']);
    expect(meaningful, isTrue);
    await expectLater(
      coordinator.persistLocations([_location(20)], isReplay: false),
      throwsStateError,
    );
  });

  test(
    'keeps the coordinator active after successful recovery when asked',
    () async {
      final written = <int>[];
      final coordinator = RecordingCoordinator(
        onLocationUpdates: (updates) async {
          written.addAll(updates.map((update) => update.location.timestampMs));
          return true;
        },
        tryAutoFinalize: () async => false,
      );

      await coordinator.recoverPending((_) async {}, remainActive: true);
      await coordinator.persistLocations([_location(10)], isReplay: false);

      expect(written, [10]);
      await coordinator.dispose();
    },
  );

  test('failed recovery leaves the coordinator inactive', () async {
    final coordinator = RecordingCoordinator(
      onLocationUpdates: (_) async => true,
    );

    await expectLater(
      coordinator.recoverPending(
        (_) async => throw StateError('recovery failed'),
        remainActive: true,
      ),
      throwsStateError,
    );
    await expectLater(
      coordinator.persistLocations([_location(10)], isReplay: true),
      throwsStateError,
    );
  });
}
