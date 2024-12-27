import 'dart:async';

import 'package:async/async.dart';
import 'package:flutter/foundation.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/map.dart';
import 'package:mutex/mutex.dart';
import 'package:notification_when_app_is_killed/model/args_for_ios.dart';
import 'package:notification_when_app_is_killed/model/args_for_kill_notification.dart';
import 'package:notification_when_app_is_killed/notification_when_app_is_killed.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps_processor.dart';
import 'package:shared_preferences/shared_preferences.dart';

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

  _loop() async {
    await Future.delayed(const Duration(minutes: 1));
    if (_running) {
      await Geolocator.getCurrentPosition(locationSettings: _locationSettings)
          // we don't care about the result
          .then((_) => null)
          .catchError((_) => null);
      _loop();
    }
  }

  cancel() {
    _running = false;
  }
}

enum GpsRecordingStatus { none, recording, paused }

class GpsRecordingState extends ChangeNotifier {
  static const String isRecordingPrefsKey = "GpsRecordingState.isRecording";
  var status = GpsRecordingStatus.none;
  Position? latestPosition;
  TrackingMode trackingMode = TrackingMode.off;

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

  // Notify the user that the recording was unexpectedly stopped on iOS.
  // On Android, this does not work, and we achive this by using foreground task.
  // On iOS we rely on this to make sure user will be notified when the app is
  // killed during recording.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

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
      _locationSettings = AppleSettings(
        accuracy: accuracy,
        distanceFilter: distanceFilter,
        // TODO: we should try to make use of `pauseLocationUpdatesAutomatically`.
        // According to doc "After a pause occurs, it’s your responsibility to
        // restart location services again".
        activityType: ActivityType.other,
        pauseLocationUpdatesAutomatically: false,
        showBackgroundLocationIndicator: false,
        allowBackgroundLocationUpdates: true,
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

  void _updatePositionStream() {
    if (_positionStream != null) return;

    _positionStream =
        Geolocator.getPositionStream(locationSettings: _locationSettings)
            .listen((Position? position) async {
      if (position != null) _onPositionUpdate(position);
    });
  }

  void _cancelPositionStream() {
    _positionStream?.cancel();
    _positionStream = null;
  }

  void _restartPositionStream() {
    _cancelPositionStream();
    _updatePositionStream();
  }

  void trackingModeChanged(TrackingMode mode) {
    if (mode != TrackingMode.off && status != GpsRecordingStatus.recording) {
      _restartPositionStream();
    } else {
      _cancelPositionStream();
      latestPosition = null;
    }
    notifyListeners();
  }

  void _saveIsRecordingState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    prefs.setBool(isRecordingPrefsKey, status == GpsRecordingStatus.recording);
  }

  void _onPositionUpdate(Position position) {
    if (status == GpsRecordingStatus.recording) {
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
      if (!(await Permission.location.isGranted ||
          await Permission.locationAlways.isGranted)) {
        return false;
      }
      return true;
    } catch (e) {
      return false;
    }
  }

  Future<void> _requestPermission() async {
    // TODO: I think there are still a lot we could improve here:
    // 1. more guidance?
    // 2. Using dialog instead of toast for some cases.
    // 3. more granular permissions?
    if (!await Geolocator.isLocationServiceEnabled()) {
      if (!await Geolocator.openLocationSettings()) {
        throw "Location services not enabled";
      }
    }

    if (await Permission.location.isPermanentlyDenied ||
        await Permission.notification.isPermanentlyDenied) {
      await Geolocator.openAppSettings();
      throw "Please allow location & notification permissions";
    }

    if (!await Permission.notification.isGranted) {
      if (!await Permission.notification.request().isGranted) {
        throw "notification permission not granted";
      }
    }

    if (!await Permission.location.isGranted) {
      if (!await Permission.location.request().isGranted) {
        throw "location permission not granted";
      }
    }

    if (!await Permission.locationAlways.isGranted) {
      // It seems this does not wait for the result on iOS, and always
      // permission is not strictly required.
      await Permission.locationAlways.request();
      if (await Permission.locationAlways.isPermanentlyDenied) {
        Fluttertoast.showToast(
            msg: "Location always permission is recommended");
      }
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
      await _notificationWhenAppIsKilledPlugin
          .cancelNotificationOnKillService();
      _pokeGeolocatorTask?.cancel();
      _pokeGeolocatorTask = null;
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
      _pokeGeolocatorTask ??= _PokeGeolocatorTask.start(_locationSettings);
      _updatePositionStream();
      await _notificationWhenAppIsKilledPlugin.setNotificationOnKillService(
        ArgsForKillNotification(
            title: 'Recording was unexpectedly stopped',
            description:
                'Recording was unexpectedly stopped, please restart the app.',
            argsForIos: ArgsForIos(
              interruptionLevel: InterruptionLevel.critical,
              useDefaultSound: true,
            )),
      );
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
