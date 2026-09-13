import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/recording_session.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps.dart';

ExtendedRawGPSPoint location(int timestamp) => ExtendedRawGPSPoint(
  rawGpsPoint: RawGPSPoint(
    point: Point(latitude: 0, longitude: 0),
    timestampMs: timestamp,
    accuracy: 1,
  ),
  receivedTimestampMs: timestamp,
);

const written = api.RecordingResult(meaningful: true, journeysFinalized: 0);

void main() {
  test(
    'wall-clock checks never finalize an active or uninitialized recording',
    () async {
      var checks = 0;
      final session = RecordingSession(
        write: (_) async => written,
        finalizePaused: () async {
          checks++;
          return true;
        },
        onJourneyFinalized: () =>
            fail('active recording must not be finalized'),
      );

      expect(await session.tryFinalizePaused(), isFalse);
      await session.start();
      await session.add(location(1));
      expect(await session.tryFinalizePaused(), isFalse);
      expect(checks, 0);
    },
  );

  test(
    'paused checks wait for acquisition and the tail of recording writes',
    () async {
      final sourceStopped = Completer<void>();
      final writingStarted = Completer<void>();
      final releaseWrite = Completer<void>();
      var checks = 0;
      final session = RecordingSession(
        write: (_) async {
          writingStarted.complete();
          await releaseWrite.future;
          return written;
        },
        finalizePaused: () async {
          checks++;
          return false;
        },
        onJourneyFinalized: () {},
      );
      await session.start();
      final write = session.add(location(1));
      await writingStarted.future;
      final stop = session.stop(() => sourceStopped.future);
      final check = session.tryFinalizePaused();
      await Future<void>.delayed(Duration.zero);
      expect(session.isPaused, isFalse);
      expect(checks, 0);
      sourceStopped.complete();
      await Future<void>.delayed(Duration.zero);
      expect(session.isPaused, isFalse);
      expect(checks, 0);
      releaseWrite.complete();
      await Future.wait([write, stop, check]);
      expect(session.isPaused, isTrue);
      expect(checks, 1);
    },
  );

  test(
    'a paused journey expires without another location, only once',
    () async {
      var expired = false;
      var notifications = 0;
      var checks = 0;
      final session = RecordingSession(
        finalizePaused: () async {
          checks++;
          return expired;
        },
        onJourneyFinalized: () {
          notifications++;
        },
      )..restorePaused();

      expect(await session.tryFinalizePaused(), isFalse);
      expired = true;
      expect(await session.tryFinalizePaused(), isTrue);
      expect(await session.tryFinalizePaused(), isFalse);
      expect(checks, 2);
      expect(notifications, 1);
      expect(session.isPaused, isFalse);
    },
  );

  test(
    'resume checks an expired paused journey before accepting new data',
    () async {
      final events = <String>[];
      final session = RecordingSession(
        write: (_) async {
          events.add('write');
          return written;
        },
        finalizePaused: () async {
          events.add('check');
          return true;
        },
        onJourneyFinalized: () {
          events.add('finalized');
        },
      )..restorePaused();

      await session.start();
      await session.add(location(1));
      expect(await session.tryFinalizePaused(), isFalse);
      expect(events, ['check', 'finalized', 'write']);
    },
  );

  test(
    'resume before expiry continues the same journey and ignores a queued tick',
    () async {
      var checks = 0;
      final session = RecordingSession(
        write: (_) async => written,
        finalizePaused: () async {
          checks++;
          return false;
        },
        onJourneyFinalized: () => fail('unexpired journey must remain open'),
      )..restorePaused();

      final resume = session.start();
      final tick = session.tryFinalizePaused();
      await resume;
      expect(await tick, isFalse);
      expect(checks, 1);
      await session.add(location(1));
    },
  );

  test('a failed source stop never enables wall-clock finalization', () async {
    final session = RecordingSession(
      write: (_) async => written,
      finalizePaused: () => throw StateError('must not be called'),
      onJourneyFinalized: () {},
    );
    await session.start();
    await expectLater(
      session.stop(() async {
        throw StateError('stop failed');
      }),
      throwsStateError,
    );
    expect(session.isPaused, isFalse);
    expect(await session.tryFinalizePaused(), isFalse);
    await session.add(location(1));
  });

  test(
    'failed writes remain pending and must succeed before pause is confirmed',
    () async {
      var failWrite = true;
      final batches = <List<int>>[];
      final session = RecordingSession(
        write: (batch) async {
          batches.add(
            batch.map((point) => point.rawGpsPoint.timestampMs!).toList(),
          );
          if (failWrite) throw StateError('write failed');
          return written;
        },
        finalizePaused: () async => false,
        onJourneyFinalized: () {},
      );
      await session.start();
      await expectLater(session.add(location(1)), throwsStateError);
      await expectLater(session.stop(() async {}), throwsStateError);
      expect(session.isPaused, isFalse);
      expect(session.needsStop, isTrue);
      expect(await session.tryFinalizePaused(), isFalse);
      await expectLater(session.start(), throwsStateError);
      await expectLater(session.finish(), throwsStateError);
      failWrite = false;
      await session.stop(() async {});
      expect(session.isPaused, isTrue);
      expect(session.needsStop, isFalse);
      expect(batches, [
        [1],
        [1],
        [1],
      ]);
    },
  );

  test(
    'locations accepted during stop are written before finalization',
    () async {
      final events = <String>[];
      final session = RecordingSession(
        write: (batch) async {
          events.add('write:${batch.single.rawGpsPoint.timestampMs}');
          return written;
        },
        finalize: () async {
          events.add('finish');
          return true;
        },
        onJourneyFinalized: () {
          events.add('finalized');
        },
      );
      await session.start();
      await session.stop(() async {
        await session.add(location(2));
      });
      expect(await session.finish(), isTrue);
      expect(events, ['write:2', 'finish', 'finalized']);
      await expectLater(session.add(location(3)), throwsStateError);
    },
  );

  test(
    'an add during writer completion waits for its own persisted data',
    () async {
      final timestamps = <int>[];
      final additionalWrite = Completer<void>();
      late RecordingSession session;
      session = RecordingSession(
        write: (batch) async {
          timestamps.addAll(
            batch.map((point) => point.rawGpsPoint.timestampMs!),
          );
          if (timestamps.length == 1) {
            scheduleMicrotask(
              () => scheduleMicrotask(() {
                session
                    .add(location(2))
                    .then(
                      (_) => additionalWrite.complete(),
                      onError: additionalWrite.completeError,
                    );
              }),
            );
          }
          return written;
        },
        onJourneyFinalized: () {},
      );
      await session.start();
      await session.add(location(1));
      await additionalWrite.future;
      expect(timestamps, [1, 2]);
    },
  );

  test(
    'a finalized journey in a draining batch is not a completed pause',
    () async {
      final pausedAtNotifications = <bool>[];
      late RecordingSession session;
      session = RecordingSession(
        write: (_) async =>
            const api.RecordingResult(meaningful: true, journeysFinalized: 2),
        finalizePaused: () async => true,
        onJourneyFinalized: () {
          pausedAtNotifications.add(session.isPaused);
        },
      );
      await session.start();
      await session.stop(() async {
        await session.add(location(1));
      });
      await session.tryFinalizePaused();
      expect(pausedAtNotifications, [false, false]);
    },
  );
}
