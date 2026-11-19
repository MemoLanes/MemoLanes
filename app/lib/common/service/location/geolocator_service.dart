import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';

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

class GeoLocatorService implements ILocationService {
  StreamSubscription<Position>? _positionStreamSub;
  _PokeGeolocatorTask? _pokeTask;
  Timer? _bufferFlushTimer;
  int _bufferDelayMs = 100;
  int? _lastReceivedTimestampMs;

  final _locationUpdateController = StreamController<LocationData>.broadcast();

  final _bufferClock = Stopwatch()..start();
  final _buffer = <(LocationData, int)>[];

  @override
  LocationBackend get locationBackend => LocationBackend.native;

  @override
  Future<void> startLocationUpdates(bool enableBackground) async {
    final settings = _buildLocationSettings(enableBackground);
    // shortcut: reorder within one second of receipt, extend only if the providers need a longer window.
    _bufferDelayMs = enableBackground ? 1000 : 100;

    _positionStreamSub =
        Geolocator.getPositionStream(locationSettings: settings).listen(
          _onPositionReceived,
          onError: (e) {
            log.error("[GeoLocatorService] getPositionStream error: $e");
          },
        );

    _pokeTask = _PokeGeolocatorTask.start(settings);
  }

  @override
  Future<void> stopLocationUpdates() async {
    await _positionStreamSub?.cancel();
    _positionStreamSub = null;

    _pokeTask?.cancel();
    _pokeTask = null;

    _flushBuffer(drain: true);
    // Let listeners enqueue the final buffered positions before stop returns.
    await Future<void>.delayed(Duration.zero);
  }

  void _onPositionReceived(Position pos) {
    final data = LocationData(
      latitude: pos.latitude,
      longitude: pos.longitude,
      accuracy: pos.accuracy,
      timestampMs: pos.timestamp.millisecondsSinceEpoch,
      altitude: pos.altitude,
      speed: pos.speed,
    );

    // Preserve receipt order across a clock change.
    final lastReceived = _lastReceivedTimestampMs;
    if (lastReceived != null && data.receivedTimestampMs < lastReceived) {
      _flushBuffer(drain: true);
    }
    _lastReceivedTimestampMs = data.receivedTimestampMs;
    _buffer.add((data, _bufferClock.elapsedMilliseconds));
    _bufferFlushTimer ??= Timer(
      Duration(milliseconds: _bufferDelayMs),
      _flushBuffer,
    );
  }

  void _flushBuffer({bool drain = false}) {
    _bufferFlushTimer?.cancel();
    _bufferFlushTimer = null;
    _buffer.sort((a, b) => a.$1.timestampMs.compareTo(b.$1.timestampMs));

    final readyBy = _bufferClock.elapsedMilliseconds - _bufferDelayMs;
    var sent = 0;
    for (final (fix, receivedAt) in _buffer) {
      if (!drain && receivedAt > readyBy) {
        // Keep later timestamps queued until this older fix can be sent.
        _bufferFlushTimer = Timer(
          Duration(milliseconds: receivedAt - readyBy),
          _flushBuffer,
        );
        break;
      }
      _locationUpdateController.add(fix);
      sent++;
    }
    _buffer.removeRange(0, sent);
  }

  @override
  StreamSubscription<LocationData> onLocationUpdate(
    void Function(LocationData) callback,
  ) {
    return _locationUpdateController.stream.listen(callback);
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

  void dispose() {
    stopLocationUpdates();
    _locationUpdateController.close();
  }
}
