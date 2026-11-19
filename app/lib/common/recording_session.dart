import 'dart:async';
import 'dart:collection';

import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps.dart';
import 'package:mutex/mutex.dart';

/// Serializes recording writes and explicit recording transitions. Paused
/// wall-clock checks are enabled only after acquisition stops and writes drain.
class RecordingSession {
  RecordingSession({
    Future<api.RecordingResult> Function(List<ExtendedRawGPSPoint>)? write,
    Future<bool> Function()? finalizePaused,
    Future<bool> Function()? finalize,
    required this.onJourneyFinalized,
  }) : _write = write ?? ((updates) => api.onLocationUpdates(updates: updates)),
       _finalizePaused = finalizePaused ?? api.tryFinalizePausedJourney,
       _finalize = finalize ?? api.finalizeOngoingJourney;

  final Future<api.RecordingResult> Function(List<ExtendedRawGPSPoint>) _write;
  final Future<bool> Function() _finalizePaused;
  final Future<bool> Function() _finalize;
  final void Function() onJourneyFinalized;

  final _transitions = Mutex();
  final _pending = Queue<ExtendedRawGPSPoint>();
  Future<void>? _writing;
  bool _accepting = false;
  bool _paused = false;
  bool _stopPending = false;

  bool get isPaused => _paused;
  bool get needsStop => _stopPending;

  /// Marks a recording paused before acquisition has started.
  void restorePaused() {
    assert(!_accepting && _pending.isEmpty);
    _paused = true;
  }

  Future<void> start() => _transitions.protect(() async {
    if (_stopPending) {
      throw StateError('Complete the pending stop before resuming');
    }
    if (_accepting) return;
    await _tryFinalizePaused();
    _paused = false;
    _accepting = true;
  });

  Future<void> add(ExtendedRawGPSPoint update) {
    if (!_accepting) {
      return Future.error(StateError('Recording is not accepting locations'));
    }
    _pending.add(update);
    return _drain();
  }

  Future<void> _drain() async {
    while (_pending.isNotEmpty || _writing != null) {
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
      final result = await _write(batch);
      // Retain the whole batch on failure. Rust commits a batch atomically;
      // its persisted cursor also makes an interrupted acknowledgement safe.
      for (var i = 0; i < batch.length; i++) {
        _pending.removeFirst();
      }
      if (result.journeysFinalized > 0) onJourneyFinalized();
    }
  }

  Future<void> stop(Future<void> Function() stopAcquisition) =>
      _transitions.protect(() async {
        _stopPending = true;
        _paused = false;
        await stopAcquisition();
        _accepting = false;
        await _drain();
        _stopPending = false;
        _paused = true;
      });

  Future<bool> tryFinalizePaused() => _transitions.protect(_tryFinalizePaused);

  Future<bool> _tryFinalizePaused() async {
    if (!_paused) return false;
    final finalized = await _finalizePaused();
    if (finalized) {
      _paused = false;
      onJourneyFinalized();
    }
    return finalized;
  }

  Future<bool> finish() => _transitions.protect(() async {
    if (_accepting || _stopPending) {
      throw StateError('Stop recording acquisition before finalizing');
    }
    await _drain();
    final finalized = await _finalize();
    if (finalized) onJourneyFinalized();
    _paused = false;
    return finalized;
  });
}
