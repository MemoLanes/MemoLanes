import 'dart:async';
import 'dart:collection';

import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps.dart';

class RecordingQueue {
  RecordingQueue({
    Future<bool> Function(List<ExtendedRawGPSPoint>)? write,
    required this.onJourneyFinalized,
  }) : _write = write ?? ((updates) => api.onLocationUpdates(updates: updates));

  final Future<bool> Function(List<ExtendedRawGPSPoint>) _write;
  final void Function() onJourneyFinalized;

  final _pending = Queue<ExtendedRawGPSPoint>();
  Future<void>? _writing;

  bool get hasPending => _pending.isNotEmpty || _writing != null;

  Future<void> add(ExtendedRawGPSPoint update) {
    _pending.add(update);
    return drain();
  }

  Future<void> drain() async {
    while (hasPending) {
      await (_writing ??= _writePending().whenComplete(() {
        _writing = null;
      }));
      // An add can arrive while the previous writer is completing. Wait for
      // that tail too before acknowledging the write or confirming a pause.
    }
  }

  Future<void> _writePending() async {
    while (_pending.isNotEmpty) {
      final batch = _pending.take(1024).toList(growable: false);
      final journeyFinalized = await _write(batch);
      // Keep the batch queued until the Rust transaction succeeds.
      for (var i = 0; i < batch.length; i++) {
        _pending.removeFirst();
      }
      if (journeyFinalized) onJourneyFinalized();
    }
  }
}
