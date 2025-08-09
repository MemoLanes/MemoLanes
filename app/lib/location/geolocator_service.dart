import 'dart:async';

import 'package:async/async.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:geolocator/geolocator.dart';

import 'location_service.dart';
import '../../logger.dart';

/// `PokeGeolocatorTask` is a hacky workround.
/// The behvior we observe is that the position stream from geolocator will
/// randomly pauses so updates are delayed and come in as a batch later.
/// However, if there something request the location, even if it is in
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
  Timer? _tooOldTimer;
  RestartableTimer? _bufferFlushTimer;

  final _locationUpdateController = StreamController<LocationData>.broadcast();

  LocationData? _latestLocation;
  final List<LocationData> _buffer = [];
  DateTime? _firstBufferReceiveTime;

  @override
  LocationBackend get locationBackend => LocationBackend.native;

  @override
  Future<void> startLocationUpdates(bool enableBackground) async {
    final settings = _buildLocationSettings(enableBackground);

    _positionStreamSub =
        Geolocator.getPositionStream(locationSettings: settings)
            .listen(_onPositionReceived, onError: (e) {
      log.error("[GeoLocatorService] getPositionStream error: $e");
    });

    _pokeTask = _PokeGeolocatorTask.start(settings);

    _tooOldTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      final now = DateTime.now().millisecondsSinceEpoch;
      final ts = _latestLocation?.timestampMs;
      if (ts != null && now - ts > 5000) {
        _latestLocation = null;
      }
    });
  }

  @override
  Future<void> stopLocationUpdates() async {
    await _positionStreamSub?.cancel();
    _positionStreamSub = null;

    _pokeTask?.cancel();
    _pokeTask = null;

    _tooOldTimer?.cancel();
    _tooOldTimer = null;

    _bufferFlushTimer?.cancel();
    _bufferFlushTimer = null;

    _buffer.clear();
    _firstBufferReceiveTime = null;
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

    final sorted = List<LocationData>.from(_buffer)
      ..sort((a, b) => a.timestampMs.compareTo(b.timestampMs));

    for (final loc in sorted) {
      _locationUpdateController.add(loc);
    }

    _buffer.clear();
    _firstBufferReceiveTime = null;
  }

  @override
  StreamSubscription<LocationData> onLocationUpdate(
      void Function(LocationData) callback) {
    return _locationUpdateController.stream.listen(callback);
  }

  LocationSettings? _buildLocationSettings(bool enableBackground) {
    const accuracy = LocationAccuracy.best;
    const distanceFilter = 0;
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        var foregroundNotificationConfig = ForegroundNotificationConfig(
          notificationChannelName: tr(
              "location_service.android_foreground_notification_channel_name"),
          notificationTitle:
              tr("location_service.android_foreground_notification_title"),
          notificationText:
              tr("location_service.android_foreground_notification_text"),
          setOngoing: true,
          enableWakeLock: true,
        );
        return AndroidSettings(
          accuracy: accuracy,
          distanceFilter: distanceFilter,
          forceLocationManager: false,
          // 1 sec feels like a reasonable interval
          intervalDuration: const Duration(seconds: 1),
          foregroundNotificationConfig:
              (enableBackground) ? foregroundNotificationConfig : null,
        );
      case TargetPlatform.iOS || TargetPlatform.macOS:
        return AppleSettings(
          accuracy: accuracy,
          distanceFilter: distanceFilter,
          // TODO: we should try to make use of `pauseLocationUpdatesAutomatically`.
          // According to doc "After a pause occurs, it’s your responsibility to
          // restart location services again".
          activityType: ActivityType.other,
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
