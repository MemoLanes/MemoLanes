import 'dart:async';
import 'dart:developer';

import 'package:async/async.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/main.dart';
import 'package:memolanes/settings.dart';
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

// `recording` requires background location but `justForTracking` does not.
enum _InternalState { off, recording, justForTracking }

bool _positionTooOld(Position position) {
  return DateTime.now().difference(position.timestamp).inSeconds >= 5;
}

class GpsManager extends ChangeNotifier {
  static const String isRecordingPrefsKey = "GpsManager.isRecording";
  var recordingStatus = GpsRecordingStatus.none;
  var mapTracking = false;
  Position? latestPosition;

  // Keep tracking of the actual internal state which represents the state of
  // gps stream. This is derived from `recordingStatus` and `mapTracking`.
  _InternalState _internalState = _InternalState.off;

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

  Timer? _lastPositionTooOldTimer;

  // Notify the user that the recording was unexpectedly stopped on iOS.
  // On Android, this does not work, and we achive this by using foreground task.
  // On iOS we rely on this to make sure user will be notified when the app is
  // killed during recording.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

  GpsManager() {
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
      if (recordState != null &&
          recordState == true &&
          await _checkLocationPermission()) {
        recordingStatus = GpsRecordingStatus.recording;
      } else {
        if (await api.hasOngoingJourney()) {
          recordingStatus = GpsRecordingStatus.paused;
        }
      }
      await _syncInternalStateWithoutLock();
    });
  }

  LocationSettings? _locationSettings(bool enableBackground) {
    const accuracy = LocationAccuracy.best;
    const distanceFilter = 0;
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        const foregroundNotificationConfig = ForegroundNotificationConfig(
          notificationText:
              "Example app will continue to receive your position even when you aren't using it",
          notificationTitle: "Running in Background",
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

  void _saveIsRecordingState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    prefs.setBool(
        isRecordingPrefsKey, recordingStatus == GpsRecordingStatus.recording);
  }

  void _onPositionUpdate(Position position) {
    // When starting the location stream, the system may give us the last known
    // location, which is a bit misleading.
    // TODO: I guess doing this is still not fully correct: when the last known
    // location is not too old but still before the time the user starts
    // recording.
    if (_positionTooOld(position)) {
      return;
    }

    latestPosition = position;
    notifyListeners();

    if (_internalState != _InternalState.recording) return;
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

  Future<void> _flushPositionBuffer() async {
    if (_positionBuffer.isEmpty) return;

    List<RawData> rawDataList = _positionBuffer
        .map((position) => RawData(
            point: Point(
              latitude: position.latitude,
              longitude: position.longitude,
            ),
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

  Future<bool> _checkLocationPermission() async {
    try {
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

  Future<void> _requestNotificationPermission() async {
    final status = await Permission.notification.status;

    if (status.isGranted) {
      await PreferencesManager.setNotificationStatus(true);
      return;
    }

    var context = navigatorKey.currentState?.context;
    if (context != null && context.mounted) {
      await showDialog(
        context: context,
        barrierDismissible: false,
        builder: (BuildContext context) {
          return AlertDialog(
            title: Text(context.tr('permission.tips')),
            content: Text(context.tr('permission.notification_reason')),
            actions: [
              TextButton(
                child: Text(context.tr('permission.i_know')),
                onPressed: () => Navigator.of(context).pop(),
              ),
            ],
          );
        },
      );
    }

    final result = await Permission.notification.request();

    if (result.isGranted) {
      await PreferencesManager.setNotificationStatus(true);
    } else {
      await PreferencesManager.setNotificationStatus(false);
    }
  }

  Future<void> _requestLocationPermission() async {
    // TODO: I think there are still a lot we could improve here:
    // 1. more guidance?
    // 2. Using dialog instead of toast for some cases.
    // 3. more granular permissions?
    if (!await Geolocator.isLocationServiceEnabled()) {
      if (!await Geolocator.openLocationSettings()) {
        throw "Location services not enabled";
      }
    }

    if (await Permission.location.isPermanentlyDenied) {
      var context = navigatorKey.currentState?.context;
      if (context != null && context.mounted) {
        await showDialog(
          context: context,
          barrierDismissible: false,
          builder: (BuildContext context) {
            return AlertDialog(
              title: Text(context.tr('permission.tips')),
              content: Text(context.tr('permission.position_not_allowed')),
              actions: [
                TextButton(
                  child: Text(context.tr('permission.i_know')),
                  onPressed: () => Navigator.of(context).pop(),
                ),
              ],
            );
          },
        );
      }
      await Geolocator.openAppSettings();
      throw "Please allow location permissions";
    }

    if (!await Permission.location.isGranted) {
      if (!await Permission.location.request().isGranted) {
        var context = navigatorKey.currentState?.context;
        if (context != null && context.mounted) {
          await showDialog(
            context: context,
            barrierDismissible: false,
            builder: (BuildContext context) {
              return AlertDialog(
                title: Text(context.tr('permission.tips')),
                content: Text(context.tr('permission.position_not_allowed')),
                actions: [
                  TextButton(
                    child: Text(context.tr('permission.i_know')),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              );
            },
          );
          throw "location permission not granted";
        }
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
      if (recordingStatus == GpsRecordingStatus.paused) {
        recordingStatus = GpsRecordingStatus.none;
        notifyListeners();
        await _syncInternalStateWithoutLock();
      }
    }
  }

  Future<void> _syncInternalStateWithoutLock() async {
    var newState = switch (recordingStatus) {
      GpsRecordingStatus.recording => _InternalState.recording,
      GpsRecordingStatus.paused || GpsRecordingStatus.none => switch (
            mapTracking) {
          true => _InternalState.justForTracking,
          false => _InternalState.off,
        },
    };
    var oldState = _internalState;
    if (oldState != newState) {
      // state changed

      // turnning off if needed
      if (oldState != _InternalState.off) {
        log("[GpsManager] turning off gps stream. old state: $oldState");
        if (oldState == _InternalState.recording) {
          await _notificationWhenAppIsKilledPlugin
              .cancelNotificationOnKillService();
        }
        await _positionStream?.cancel();
        _pokeGeolocatorTask?.cancel();
        _pokeGeolocatorTask = null;
        _positionStream = null;
        _positionBufferFlushTimer?.cancel();
        _positionBufferFlushTimer = null;
        _lastPositionTooOldTimer?.cancel();
        _lastPositionTooOldTimer = null;
        await _flushPositionBuffer();
        if (newState == _InternalState.off) {
          latestPosition = null;
        }
      }

      // turnning on if needed
      if (newState != _InternalState.off) {
        log("[GpsManager] turning on gps stream. new state: $newState");
        var locationSettings =
            _locationSettings(newState == _InternalState.recording);

        _positionStream ??=
            Geolocator.getPositionStream(locationSettings: locationSettings)
                .listen((Position? position) async {
          if (position != null) {
            _onPositionUpdate(position);
          }
        });
        _pokeGeolocatorTask ??= _PokeGeolocatorTask.start(locationSettings);
        _lastPositionTooOldTimer ??=
            Timer.periodic(Duration(seconds: 1), (timer) {
          var latestPosition = this.latestPosition;
          if (latestPosition != null) {
            if (_positionTooOld(latestPosition)) {
              this.latestPosition = null;
              notifyListeners();
            }
          }
        });
        if (newState == _InternalState.recording &&
            await PreferencesManager.getNotificationStatus()) {
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
      }
      _internalState = newState;
      notifyListeners();
    }
  }

  Future<bool> _checkAndRequestPermission() async {
    try {
      if (await _checkLocationPermission()) {
        return true;
      }
      await _requestLocationPermission();
      await _requestNotificationPermission();
      var hasPermission = await _checkLocationPermission();
      if (!hasPermission) {
        Fluttertoast.showToast(msg: "Permission not granted");
      }
      return hasPermission;
    } catch (e) {
      Fluttertoast.showToast(msg: e.toString());
      return false;
    }
  }

  Future<void> changeRecordingState(GpsRecordingStatus to) async {
    if (to == GpsRecordingStatus.recording) {
      if (!await _checkAndRequestPermission()) {
        return;
      }
    }

    await _m.protect(() async {
      var needToFinalize =
          recordingStatus != to && to == GpsRecordingStatus.none;
      recordingStatus = to;
      notifyListeners();

      await _syncInternalStateWithoutLock();
      _saveIsRecordingState();
      if (needToFinalize) {
        if (await api.finalizeOngoingJourney()) {
          Fluttertoast.showToast(msg: "New journey added");
        } else {
          Fluttertoast.showToast(msg: "No journey detected");
        }
      }
    });
  }

  Future<void> toggleMapTracking(bool enable) async {
    if (enable) {
      if (!await _checkAndRequestPermission()) {
        return;
      }
    }

    await _m.protect(() async {
      mapTracking = enable;
      await _syncInternalStateWithoutLock();
    });
  }
}
