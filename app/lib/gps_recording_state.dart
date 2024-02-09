import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:geolocator/geolocator.dart';
import 'package:mutex/mutex.dart';
import 'package:permission_handler/permission_handler.dart';

import 'package:project_dv/src/rust/api/api.dart';

/// `PokeGeolocatorTask` is a hacky workround on Android.
/// The behvior we observe is that the position stream from geolocator will
/// randomly pauses so updates are delayed or missed even when holding the
/// wakelock. However, if there something request the location, even if it is in
/// another app, the stream will resume. So the hack is to poke the geolocator
/// frequently.
class _PokeGeolocatorTask {
  // TODO: Test on iOS
  bool running = false;
  _PokeGeolocatorTask();

  factory _PokeGeolocatorTask.start() {
    var task = _PokeGeolocatorTask();
    task.running = true;
    if (defaultTargetPlatform == TargetPlatform.android) {
      task._loop();
    }
    return task;
  }

  _loop() async {
    await Future.delayed(const Duration(seconds: 5));
    if (running) {
      await Geolocator.getCurrentPosition(
              timeLimit: const Duration(seconds: 10))
          // we don't care about the result
          .then((_) => null)
          .catchError((_) => null);
      _loop();
    }
  }

  cancel() {
    running = false;
  }
}

class GpsRecordingState extends ChangeNotifier {
  var isRecording = false;
  Position? latestPosition;

  LocationSettings? _locationSettings;
  StreamSubscription<Position>? _positionStream;
  _PokeGeolocatorTask? _pokeGeolocatorTask;
  final Mutex _m = Mutex();

  GpsRecordingState() {
    var accuracy = LocationAccuracy.best;
    var distanceFilter = 0;
    if (defaultTargetPlatform == TargetPlatform.android) {
      _locationSettings = AndroidSettings(
          accuracy: accuracy,
          distanceFilter: distanceFilter,
          forceLocationManager: false,
          // 1 sec feels like a reasonable interval
          intervalDuration: const Duration(seconds: 1),
          foregroundNotificationConfig: const ForegroundNotificationConfig(
            notificationText:
                "Example app will continue to receive your position even when you aren't using it",
            notificationTitle: "Running in Background",
            enableWakeLock: false,
          ));
    } else if (defaultTargetPlatform == TargetPlatform.iOS ||
        defaultTargetPlatform == TargetPlatform.macOS) {
      // TODO: not tested on iOS, it is likely that we need to tweak the
      // settings.
      _locationSettings = AppleSettings(
        accuracy: accuracy,
        activityType: ActivityType.fitness,
        distanceFilter: distanceFilter,
        pauseLocationUpdatesAutomatically: true,
        showBackgroundLocationIndicator: false,
      );
    } else {
      _locationSettings = LocationSettings(
        accuracy: accuracy,
        distanceFilter: distanceFilter,
      );
    }
  }

  Future<void> _onLocationUpdate(Position position) async {
    if (!isRecording) return;
    latestPosition = position;
    notifyListeners();

    await onLocationUpdate(
        latitude: position.latitude,
        longitude: position.longitude,
        timestampMs: position.timestamp.millisecondsSinceEpoch,
        accuracy: position.accuracy,
        altitude: position.altitude,
        speed: position.speed);
  }

  Future<bool> _hasLocationPermission() async {
    return await Permission.locationAlways.isGranted ||
        await Permission.locationWhenInUse.isGranted;
  }

  void toggle() async {
    await _m.protect(() async {
      if (isRecording) {
        await _positionStream?.cancel();
        _positionStream = null;
        _pokeGeolocatorTask?.cancel();
        _pokeGeolocatorTask = null;
        latestPosition = null;
      } else {
        if (!await _hasLocationPermission()) {
          await Permission.locationAlways.request();
          if (!await _hasLocationPermission()) {
            throw const FormatException("Location permission not granted");
          }
        }
        _pokeGeolocatorTask ??= _PokeGeolocatorTask.start();
        _positionStream ??=
            Geolocator.getPositionStream(locationSettings: _locationSettings)
                .listen((Position? position) async {
          if (position != null) {
            await _onLocationUpdate(position);
          }
        });
      }
      isRecording = !isRecording;
      notifyListeners();
    });
  }
}
