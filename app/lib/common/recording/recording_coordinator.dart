import 'dart:async';

import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps_processor.dart';
import 'package:mutex/mutex.dart';

typedef RecordingLocationUpdate = ({
  LocationData location,
  DateTime receivedAt,
});

/// Owns the provider-neutral ordering boundary into Rust recording.
///
/// Live deliveries and durable replay batches both enter here, making this the
/// single place that serializes the stateful GPS preprocessor.
class RecordingCoordinator {
  RecordingCoordinator({
    Future<void> Function()? onJourneyFinalized,
    Future<bool> Function(List<RecordingLocationUpdate> updates)?
    onLocationUpdates,
    Future<bool> Function()? tryAutoFinalize,
    DateTime Function()? now,
  }) : _tryAutoFinalizeOverride = tryAutoFinalize,
       _now = now ?? DateTime.now {
    _onJourneyFinalized = onJourneyFinalized;
    _onLocationUpdates = onLocationUpdates;
  }

  late final Future<void> Function()? _onJourneyFinalized;
  late final Future<bool> Function(List<RecordingLocationUpdate> updates)?
  _onLocationUpdates;
  final Future<bool> Function()? _tryAutoFinalizeOverride;
  final DateTime Function() _now;

  // Every Rust preprocessor operation uses one mutex and one lock order.
  // Finalization callbacks run only after this mutex has been released.
  final _operationMutex = Mutex();

  bool _recording = false;
  DateTime? _lastMeaningfulLiveUpdate;

  Future<void> start() => _operationMutex.protect(() async {
    if (_recording) return;
    _recording = true;
    _lastMeaningfulLiveUpdate = null;
  });

  /// Restores a provider-owned backlog before live recording starts.
  ///
  /// The provider controls how data is stored and replayed. This coordinator
  /// only supplies the common recording consumer and preserves the required
  /// recovery-before-finalization ordering.
  Future<bool> recoverPending(
    Future<void> Function(LocationBatchConsumer consumer) recover, {
    required bool remainActive,
  }) async {
    await start();
    var completed = false;
    var meaningful = false;
    try {
      await recover((locations, {required bool isReplay}) async {
        if (await persistLocations(locations, isReplay: isReplay)) {
          meaningful = true;
        }
      });
      await tryAutoFinalize();
      completed = true;
      return meaningful;
    } finally {
      if (!completed || !remainActive) await stop();
    }
  }

  /// Persists a batch and returns whether Rust accepted a meaningful point.
  ///
  /// The input is stably ordered by provider timestamp. A `false` return still
  /// means the batch was persisted successfully; only a thrown error means a
  /// durable provider must retain its data for retry.
  Future<bool> persistLocations(
    List<LocationData> locations, {
    required bool isReplay,
  }) async {
    var finalized = false;
    final meaningful = await _operationMutex.protect(() async {
      if (!_recording) {
        throw StateError('Recording coordinator is not active');
      }

      final ordered = sortLocationDataByTimestamp(locations);
      final updates = ordered
          .map((location) => (location: location, receivedAt: _now()))
          .toList(growable: false);
      if (updates.isEmpty) return false;

      final receivedAt = updates.first.receivedAt;
      final lastMeaningful = _lastMeaningfulLiveUpdate;
      if (!isReplay &&
          lastMeaningful != null &&
          receivedAt.difference(lastMeaningful).inSeconds >= 60) {
        finalized = await _tryFinalizeJourney();
        // Avoid retrying auto-finalize for every subsequent ignored point.
        _lastMeaningfulLiveUpdate = receivedAt;
      }

      final locationUpdates = _onLocationUpdates;
      final meaningful = locationUpdates == null
          ? await api.onLocationUpdates(
              updates: updates
                  .map(
                    (update) => api.LocationUpdate(
                      rawData: _rawData(update.location),
                      receivedTimestampMs:
                          update.receivedAt.millisecondsSinceEpoch,
                    ),
                  )
                  .toList(growable: false),
            )
          : await locationUpdates(updates);

      if (!isReplay && meaningful) {
        _lastMeaningfulLiveUpdate = updates.last.receivedAt;
      }
      return meaningful;
    });
    if (finalized) _dispatchJourneyFinalized();
    return meaningful;
  }

  /// Stops accepting data after every earlier write has completed.
  Future<void> stop() => _operationMutex.protect(() async {
    _recording = false;
  });

  Future<bool> _tryFinalizeJourney() async {
    final tryAutoFinalize = _tryAutoFinalizeOverride;
    final finalized = tryAutoFinalize == null
        ? await api.tryAutoFinalizeJourney()
        : await tryAutoFinalize();
    return finalized;
  }

  Future<void> tryAutoFinalize() async {
    final finalized = await _operationMutex.protect(() async {
      final result = await _tryFinalizeJourney();
      _lastMeaningfulLiveUpdate = _now();
      return result;
    });
    if (finalized) _dispatchJourneyFinalized();
  }

  void _dispatchJourneyFinalized() {
    final callback = _onJourneyFinalized;
    if (callback == null) return;
    scheduleMicrotask(() async {
      try {
        await callback();
      } catch (error, stackTrace) {
        log.error(
          '[RecordingCoordinator] journey-finalized callback failed: $error',
          stackTrace,
        );
      }
    });
  }

  RawData _rawData(LocationData location) => RawData(
    point: Point(latitude: location.latitude, longitude: location.longitude),
    timestampMs: location.timestampMs,
    accuracy: location.accuracy,
    altitude: location.altitude,
    speed: location.speed,
  );

  Future<void> dispose() => stop();
}
