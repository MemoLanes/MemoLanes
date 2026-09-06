import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps_processor.dart';
import 'package:mutex/mutex.dart';

typedef RecordingLocationUpdate = ({
  LocationData location,
  DateTime receivedAt,
});

const _persistedKeyCapacity = 8192;

/// Owns the provider-neutral ordering boundary into Rust recording.
///
/// Live deliveries and durable replay batches both enter here, making this the
/// single place that serializes the stateful GPS preprocessor and deduplicates
/// points that arrive through both paths.
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
  // Dart set literals preserve insertion order, which is the LRU order here.
  final _persistedKeys = <String>{};

  bool _recording = false;
  DateTime? _lastMeaningfulLiveUpdate;

  Future<void> start() => _operationMutex.protect(() async {
    if (_recording) return;
    _recording = true;
    _persistedKeys.clear();
    _lastMeaningfulLiveUpdate = null;
  });

  /// Persists a batch and returns whether Rust accepted a meaningful point.
  ///
  /// The input is stably ordered by provider timestamp. A `false` return still
  /// means the batch was persisted successfully; only a thrown error means a
  /// durable provider must retain its data for retry.
  Future<bool> persistLocations(LocationRecordingBatch batch) async {
    var finalized = false;
    final meaningful = await _operationMutex.protect(() async {
      if (!_recording) {
        throw StateError('Recording coordinator is not active');
      }

      final ordered = sortLocationDataByTimestamp(batch.locations);
      final updates = <RecordingLocationUpdate>[];
      final batchKeys = <String>{};
      for (final location in ordered) {
        final key = _locationKey(location);
        // A durable cursor is the authoritative idempotency mechanism. Keep
        // its complete immutable payload so Rust can validate retries.
        if (batch.durableCursor == null &&
            (_touchPersistedKey(key) || !batchKeys.add(key))) {
          continue;
        }
        batchKeys.add(key);
        updates.add((location: location, receivedAt: _now()));
      }
      if (updates.isEmpty) return false;

      final receivedAt = updates.first.receivedAt;
      final lastMeaningful = _lastMeaningfulLiveUpdate;
      if (!batch.isReplay &&
          lastMeaningful != null &&
          receivedAt.difference(lastMeaningful).inSeconds >= 60) {
        finalized = await _tryFinalizeJourney();
        // Avoid retrying auto-finalize for every subsequent ignored point.
        _lastMeaningfulLiveUpdate = receivedAt;
      }

      final locationUpdates = _onLocationUpdates;
      final meaningful = locationUpdates == null
          ? (await api.onLocationRecordingBatch(
              batch: api.LocationRecordingBatch(
                updates: updates
                    .map(
                      (update) => api.LocationUpdate(
                        rawData: _rawData(update.location),
                        receivedTimestampMs:
                            update.receivedAt.millisecondsSinceEpoch,
                      ),
                    )
                    .toList(growable: false),
                durableCursor: switch (batch.durableCursor) {
                  null => null,
                  final cursor => api.DurableDeliveryCursor(
                    providerId: cursor.providerId,
                    streamId: cursor.streamId,
                    firstSequence: cursor.firstSequence,
                    lastSequence: cursor.lastSequence,
                  ),
                },
              ),
            )).meaningful
          : await locationUpdates(updates);

      // Remember only after the write completed. A thrown write is not an
      // acknowledgement and must remain retryable by a durable provider.
      _rememberPersistedKeys(batchKeys);
      if (!batch.isReplay && meaningful) {
        _lastMeaningfulLiveUpdate = updates.last.receivedAt;
      }
      return meaningful;
    });
    if (finalized) await _onJourneyFinalized?.call();
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
    if (finalized) await _onJourneyFinalized?.call();
  }

  String _locationKey(LocationData location) =>
      '${location.timestampMs}:${location.latitude}:${location.longitude}';

  bool _touchPersistedKey(String key) {
    if (!_persistedKeys.remove(key)) return false;
    _persistedKeys.add(key);
    return true;
  }

  void _rememberPersistedKeys(Iterable<String> keys) {
    for (final key in keys) {
      _persistedKeys.remove(key);
      _persistedKeys.add(key);
    }
    while (_persistedKeys.length > _persistedKeyCapacity) {
      _persistedKeys.remove(_persistedKeys.first);
    }
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
