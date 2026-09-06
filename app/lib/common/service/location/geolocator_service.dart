import 'dart:async';

import 'package:async/async.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:mutex/mutex.dart';

/// `PokeGeolocatorTask` is a hacky workaround.
/// The behavior we observe is that the position stream from geolocator will
/// randomly pause so updates are delayed and come in as a batch later.
/// However, if something requests the location, even if it is in
/// another app, the stream will resume. So the hack is to poke the geolocator
/// frequently.
class _PokeGeolocatorTask {
  bool _running = false;
  LocationSettings? _locationSettings;

  _PokeGeolocatorTask._();

  factory _PokeGeolocatorTask.start(LocationSettings? locationSettings) {
    var self = _PokeGeolocatorTask._();
    self._running = true;
    self._locationSettings = locationSettings;
    self._loop();
    return self;
  }

  Future<void> _loop() async {
    await Future.delayed(const Duration(minutes: 1));
    if (_running) {
      await Geolocator.getCurrentPosition(locationSettings: _locationSettings)
          // we don't care about the result
          .then((_) => null)
          .catchError((_) => null);
      _loop();
    }
  }

  void cancel() {
    _running = false;
  }
}

ILocationService createDefaultLocationService() => GeoLocatorService();

class GeoLocatorService implements ILocationService {
  final _lifecycleMutex = Mutex();
  StreamSubscription<Position>? _positionStreamSub;
  _PokeGeolocatorTask? _pokeTask;
  Timer? _tooOldTimer;
  RestartableTimer? _bufferFlushTimer;

  final _locationUpdateController = StreamController<LocationData>.broadcast();

  LocationData? _latestLocation;
  final List<LocationData> _buffer = [];
  DateTime? _firstBufferReceiveTime;
  LocationBatchConsumer? _recordingConsumer;
  Future<void> _recordingTail = Future<void>.value();
  bool _running = false;

  @override
  LocationProviderInfo get providerInfo => LocationProviderInfo.native;

  @override
  Stream<LocationData> get locations => _locationUpdateController.stream;

  @override
  Future<void> start(LocationStartOptions options) =>
      _lifecycleMutex.protect(() async {
        if (_running) return;
        _recordingConsumer = options.recordingConsumer;
        final settings = _buildLocationSettings(options.allowBackground);

        try {
          _positionStreamSub =
              Geolocator.getPositionStream(locationSettings: settings).listen(
                _onPositionReceived,
                onError: (e) {
                  log.error("[GeoLocatorService] getPositionStream error: $e");
                },
              );

          _pokeTask = _PokeGeolocatorTask.start(settings);

          _tooOldTimer = Timer.periodic(const Duration(seconds: 1), (_) {
            final now = DateTime.now().millisecondsSinceEpoch;
            final ts = _latestLocation?.timestampMs;
            if (ts != null && now - ts > 5000) {
              _latestLocation = null;
            }
          });
          _running = true;
        } catch (error, stackTrace) {
          try {
            await _stopResources(deliverBufferedLocations: false);
          } catch (cleanupError, cleanupStackTrace) {
            log.error(
              '[GeoLocatorService] partial-start cleanup failed: $cleanupError',
              cleanupStackTrace,
            );
          }
          Error.throwWithStackTrace(error, stackTrace);
        }
      });

  @override
  Future<void> setForeground(bool foreground) async {
    // This provider has no durable queue, so delivery remains live in either
    // lifecycle state. Platform background behavior is configured at start.
  }

  @override
  Future<void> recoverPendingDeliveries(LocationBatchConsumer consumer) async {}

  @override
  Future<void> stop() => _lifecycleMutex.protect(
    () => _stopResources(deliverBufferedLocations: true),
  );

  Future<void> _stopResources({required bool deliverBufferedLocations}) async {
    _running = false;
    _pokeTask?.cancel();
    _pokeTask = null;
    _tooOldTimer?.cancel();
    _tooOldTimer = null;
    _bufferFlushTimer?.cancel();
    _bufferFlushTimer = null;

    Object? firstError;
    StackTrace? firstStackTrace;
    try {
      await _positionStreamSub?.cancel();
    } catch (error, stackTrace) {
      firstError = error;
      firstStackTrace = stackTrace;
    } finally {
      _positionStreamSub = null;
    }

    if (deliverBufferedLocations) {
      _flushBuffer();
      try {
        await _recordingTail;
      } catch (error, stackTrace) {
        firstError ??= error;
        firstStackTrace ??= stackTrace;
      }
    } else {
      _buffer.clear();
      _firstBufferReceiveTime = null;
    }
    _recordingConsumer = null;
    _latestLocation = null;

    if (firstError != null) {
      Error.throwWithStackTrace(firstError, firstStackTrace!);
    }
  }

  void _onPositionReceived(Position pos) {
    final now = DateTime.now();

    final data = LocationData(
      latitude: pos.latitude,
      longitude: pos.longitude,
      accuracy: pos.accuracy,
      timestampMs: pos.timestamp.millisecondsSinceEpoch,
      altitude: pos.altitude,
      speed: pos.speed,
    );

    _latestLocation = data;
    _buffer.add(data);
    _firstBufferReceiveTime ??= now;

    if (_bufferFlushTimer == null) {
      _bufferFlushTimer = RestartableTimer(
        const Duration(milliseconds: 100),
        _flushBuffer,
      );
    } else {
      _bufferFlushTimer?.reset();
    }
  }

  void _flushBuffer() {
    if (_buffer.isEmpty) return;

    final sorted = sortLocationDataByTimestamp(_buffer);

    for (final loc in sorted) {
      _locationUpdateController.add(loc);
    }
    _enqueueRecordingBatch(sorted);

    _buffer.clear();
    _firstBufferReceiveTime = null;
  }

  void _enqueueRecordingBatch(List<LocationData> locations) {
    final consumer = _recordingConsumer;
    if (consumer == null || locations.isEmpty) return;

    _recordingTail = _recordingTail
        .then(
          (_) => consumer(
            LocationRecordingBatch(locations: locations, isReplay: false),
          ),
        )
        .catchError((Object error, StackTrace stackTrace) {
          // The native provider has no durable queue to retry from. Keep the
          // delivery chain usable and surface the failure in diagnostics.
          log.error(
            '[GeoLocatorService] recording consumer failed: $error',
            stackTrace,
          );
        });
  }

  LocationSettings? _buildLocationSettings(bool enableBackground) {
    const accuracy = LocationAccuracy.best;
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        var foregroundNotificationConfig = ForegroundNotificationConfig(
          notificationChannelName: tr(
            "location_service.android_foreground_notification_channel_name",
          ),
          notificationTitle: tr(
            "location_service.android_foreground_notification_title",
          ),
          notificationText: tr(
            "location_service.android_foreground_notification_text",
          ),
          setOngoing: true,
          enableWakeLock: true,
        );
        return AndroidSettings(
          accuracy: accuracy,
          distanceFilter: 0, // On Android, `0` means no distance filter.
          forceLocationManager: false,
          // 1 sec feels like a reasonable interval
          intervalDuration: const Duration(seconds: 1),
          foregroundNotificationConfig: (enableBackground)
              ? foregroundNotificationConfig
              : null,
        );
      case TargetPlatform.iOS || TargetPlatform.macOS:
        return AppleSettings(
          accuracy: accuracy,
          // On iOS, `-1` means no distance filter. It is different from
          // Android and a bit weird. https://github.com/Baseflow/flutter-geolocator/issues/1746
          distanceFilter: -1,
          activityType: ActivityType.other,
          // TODO: we should try to make use of `pauseLocationUpdatesAutomatically`.
          // According to doc "After a pause occurs, it’s your responsibility to
          // restart location services again".
          pauseLocationUpdatesAutomatically: false,
          showBackgroundLocationIndicator: false,
          allowBackgroundLocationUpdates: enableBackground,
        );
      case _:
        return null;
    }
  }

  @override
  Future<void> dispose() async {
    await stop();
    _locationUpdateController.close();
  }
}
