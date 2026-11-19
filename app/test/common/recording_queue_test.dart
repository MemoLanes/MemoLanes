import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/recording_queue.dart';
import 'package:memolanes/src/rust/gps.dart';

ExtendedRawGPSPoint location(int timestamp) => ExtendedRawGPSPoint(
  rawGpsPoint: RawGPSPoint(
    point: Point(latitude: 0, longitude: 0),
    timestampMs: timestamp,
  ),
  receivedTimestampMs: timestamp,
);

void main() {
  test('a failed batch is retained until the next drain succeeds', () async {
    var writes = 0;
    var notifications = 0;
    final point = location(1);
    final queue = RecordingQueue(
      write: (batch) async {
        expect(batch, [point]);
        if (++writes == 1) throw StateError('write failed');
        return true;
      },
      onJourneyFinalized: () => notifications++,
    );

    await expectLater(queue.add(point), throwsStateError);
    expect(queue.hasPending, isTrue);
    expect(notifications, 0);
    await queue.drain();
    expect(queue.hasPending, isFalse);
    expect(notifications, 1);
  });

  test('a point added during writer completion is persisted', () async {
    final timestamps = <int>[];
    final additionalWrite = Completer<void>();
    late RecordingQueue queue;
    queue = RecordingQueue(
      write: (batch) async {
        timestamps.addAll(batch.map((point) => point.rawGpsPoint.timestampMs!));
        if (timestamps.length == 1) {
          scheduleMicrotask(() {
            scheduleMicrotask(() {
              queue
                  .add(location(2))
                  .then(
                    (_) => additionalWrite.complete(),
                    onError: additionalWrite.completeError,
                  );
            });
          });
        }
        return false;
      },
      onJourneyFinalized: () => fail('unexpected journey finalization'),
    );

    await queue.add(location(1));
    await additionalWrite.future;
    expect(timestamps, [1, 2]);
    expect(queue.hasPending, isFalse);
  });
}
