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

  final _writeMutex = Mutex();
  final _finalizeMutex = Mutex();
  // Dart set literals preserve insertion order, which is the LRU order here.
  final _persistedKeys = <String>{};

  bool _recording = false;
  DateTime? _lastMeaningfulLiveUpdate;

  Future<void> start() => _writeMutex.protect(() async {
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
  Future<bool> persistLocations(
    List<LocationData> locations, {
    required bool isReplay,
  }) => _writeMutex.protect(() async {
    if (!_recording) {
      throw StateError('Recording coordinator is not active');
    }

    final ordered = sortLocationDataByTimestamp(locations);
    final updates = <RecordingLocationUpdate>[];
    final batchKeys = <String>{};
    for (final location in ordered) {
      final key = _locationKey(location);
      if (_touchPersistedKey(key) || !batchKeys.add(key)) continue;
      updates.add((location: location, receivedAt: _now()));
    }
    if (updates.isEmpty) return false;

    final receivedAt = updates.first.receivedAt;
    final lastMeaningful = _lastMeaningfulLiveUpdate;
    if (!isReplay &&
        lastMeaningful != null &&
        receivedAt.difference(lastMeaningful).inSeconds >= 60) {
      await _finalizeMutex.protect(_tryFinalizeJourney);
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

    // Remember only after the write completed. A thrown write is not an
    // acknowledgement and must remain retryable by a durable provider.
    _rememberPersistedKeys(batchKeys);
    if (!isReplay && meaningful) {
      _lastMeaningfulLiveUpdate = updates.last.receivedAt;
    }
    return meaningful;
  });

  /// Stops accepting data after every earlier write has completed.
  Future<void> stop() => _writeMutex.protect(() async {
    _recording = false;
  });

  Future<void> _tryFinalizeJourney() async {
    final tryAutoFinalize = _tryAutoFinalizeOverride;
    final finalized = tryAutoFinalize == null
        ? await api.tryAutoFinalizeJourney()
        : await tryAutoFinalize();
    if (finalized) await _onJourneyFinalized?.call();
  }

  Future<void> tryAutoFinalize() async {
    await _finalizeMutex.protect(_tryAutoFinalizeJourneyAndResetCountdown);
  }

  Future<void> _tryAutoFinalizeJourneyAndResetCountdown() async {
    await _tryFinalizeJourney();
    _lastMeaningfulLiveUpdate = _now();
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
