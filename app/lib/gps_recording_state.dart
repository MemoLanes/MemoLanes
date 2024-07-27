import 'dart:async';

import 'package:async/async.dart';
import 'package:flutter/foundation.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:mutex/mutex.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps_processor.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// `PokeGeolocatorTask` is a hacky workround on Android.
/// The behvior we observe is that the position stream from geolocator will
/// randomly pauses so updates are delayed and come in as a batch later.
/// However, if there something request the location, even if it is in
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
    await Future.delayed(const Duration(minutes: 1));
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

enum GpsRecordingStatus { none, recording, paused }

class GpsRecordingState extends ChangeNotifier {
  static const String isRecordingPrefsKey = "GpsRecordingState.isRecording";
  var status = GpsRecordingStatus.none;
  Position? latestPosition;

  LocationSettings? _locationSettings;
  StreamSubscription<Position>? _positionStream;
  final Mutex _m = Mutex();
  _PokeGeolocatorTask? _pokeGeolocatorTask;

  // NOTE: we noticed that on Andorid, location updates may delivered in batches,
  // updates within the same batch can be out of order, so we try to batch them
  // back together using this buffer. The rust code will sort updates for each
  // batch.
  RestartableTimer? _positionBufferFlushTimer;
  final List<Position> _positionBuffer = [];
  DateTime? _positionBufferFirstElementReceivedTime;

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
            setOngoing: true,
            enableWakeLock: true,
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
    _initState();
  }

  void _initState() async {
    await _m.protect(() async {
      await _tryFinalizeJourneyWithoutLock();
      Timer.periodic(const Duration(minutes: 5), (timer) async {
        await _m.protect(() async {
          await _tryFinalizeJourneyWithoutLock();
        });
      });

      SharedPreferences prefs = await SharedPreferences.getInstance();
      bool? recordState = prefs.getBool(isRecordingPrefsKey);
      if (recordState != null && recordState == true) {
        await _changeStateWithoutLock(GpsRecordingStatus.recording);
      } else {
        if (await api.hasOngoingJourney()) {
          status = GpsRecordingStatus.paused;
        }
      }
      notifyListeners();
    });
  }

  void _saveIsRecordingState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    prefs.setBool(isRecordingPrefsKey, status == GpsRecordingStatus.recording);
  }

  void _onPositionUpdate(Position position) {
    if (status != GpsRecordingStatus.recording) return;
    _positionBuffer.add(position);
    _positionBufferFirstElementReceivedTime ??= DateTime.now();
    if (_positionBufferFlushTimer == null) {
      _positionBufferFlushTimer = RestartableTimer(
        const Duration(milliseconds: 100),
        _flushPositionBuffer,
      );
    } else {
      _positionBufferFlushTimer?.reset();
    }

    latestPosition = position;
    notifyListeners();
  }

  Future<void> _flushPositionBuffer() async {
    if (_positionBuffer.isEmpty) return;

    List<RawData> rawDataList = _positionBuffer
        .map((position) => RawData(
            latitude: position.latitude,
            longitude: position.longitude,
            timestampMs: position.timestamp.millisecondsSinceEpoch,
            accuracy: position.accuracy,
            altitude: position.altitude,
            speed: position.speed))
        .toList();
    int receviedTimestampMs =
        _positionBufferFirstElementReceivedTime!.millisecondsSinceEpoch;
    _positionBufferFirstElementReceivedTime = null;
    _positionBuffer.clear();

    await api.onLocationUpdate(
        rawDataList: rawDataList, receviedTimestampMs: receviedTimestampMs);
  }

  Future<bool> _checkPermission() async {
    try {
      if (!await Permission.notification.isGranted) {
        return false;
      }
      if (!await Geolocator.isLocationServiceEnabled()) {
        return false;
      }
      if (!(await Geolocator.checkPermission() == LocationPermission.always)) {
        return false;
      }
      return true;
    } catch (e) {
      return false;
    }
  }

  Future<void> _requestPermission() async {
    if (!await Geolocator.isLocationServiceEnabled()) {
      if (!await Geolocator.openLocationSettings()) {
        throw "Location services not enabled";
      }
    }

    if (!await Permission.notification.isGranted) {
      await Permission.notification.request();
      if (!await Permission.notification.isGranted) {
        throw "notification permission not granted";
      }
    }

    LocationPermission permission = await Geolocator.requestPermission();
    if (permission == LocationPermission.whileInUse) {
      await Permission.locationAlways.request();
      permission = await Geolocator.checkPermission();
    }
    if (permission != LocationPermission.always) {
      await Geolocator.openAppSettings();
      throw "Please allow location permissions";
    }
  }

  Future<void> _tryFinalizeJourneyWithoutLock() async {
    // TODO: I think we want this to be configurable
    if (await api.tryAutoFinalizeJourny()) {
      Fluttertoast.showToast(msg: "New journey added");
      if (status == GpsRecordingStatus.paused) {
        status = GpsRecordingStatus.none;
        notifyListeners();
      }
    }
  }

  Future<void> _changeStateWithoutLock(GpsRecordingStatus to) async {
    if (status == to) return;

    if (status == GpsRecordingStatus.recording &&
        to != GpsRecordingStatus.recording) {
      // stop recording
      await _positionStream?.cancel();
      _pokeGeolocatorTask?.cancel();
      _pokeGeolocatorTask = null;
      _positionStream = null;
      latestPosition = null;
      _positionBufferFlushTimer?.cancel();
      _positionBufferFlushTimer = null;
      await _flushPositionBuffer();
    }

    if (to == GpsRecordingStatus.recording) {
      // start recording
      try {
        if (!await _checkPermission()) {
          await _requestPermission();
        }
      } catch (e) {
        Fluttertoast.showToast(msg: e.toString());
        return;
      }
      _pokeGeolocatorTask ??= _PokeGeolocatorTask.start();
      _positionStream ??=
          Geolocator.getPositionStream(locationSettings: _locationSettings)
              .listen((Position? position) async {
        if (position != null) {
          _onPositionUpdate(position);
        }
      });
    }

    if (to == GpsRecordingStatus.none) {
      if (await api.finalizeOngoingJourney()) {
        Fluttertoast.showToast(msg: "New journey added");
      } else {
        Fluttertoast.showToast(msg: "No journey detected");
      }
    }

    status = to;
    _saveIsRecordingState();
    notifyListeners();
  }

  Future<void> changeState(GpsRecordingStatus to) async {
    await _m.protect(() async {
      await _changeStateWithoutLock(to);
    });
  }
}
