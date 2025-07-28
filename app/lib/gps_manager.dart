import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/main.dart';
import 'package:memolanes/preferences_manager.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps_processor.dart';
import 'package:memolanes/utils.dart';
import 'package:mutex/mutex.dart';
import 'package:notification_when_app_is_killed/model/args_for_ios.dart';
import 'package:notification_when_app_is_killed/model/args_for_kill_notification.dart';
import 'package:notification_when_app_is_killed/notification_when_app_is_killed.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'location/geo_locator_service.dart';
import 'location/location_service.dart';
import 'logger.dart';

enum GpsRecordingStatus { none, recording, paused }

// `recording` requires background location but `justForTracking` does not.
enum _InternalState { off, recording, justForTracking }

bool _positionTooOld(LocationData data) {
  final now = DateTime.now().millisecondsSinceEpoch;
  return now - data.timestampMs >= 5000;
}

class GpsManager extends ChangeNotifier {
  static const String isRecordingPrefsKey = "GpsManager.isRecording";
  late final ILocationService _locationService;
  var recordingStatus = GpsRecordingStatus.none;
  var mapTracking = false;
  LocationData? latestPosition;

  // Keep tracking of the actual internal state which represents the state of
  // gps stream. This is derived from `recordingStatus` and `mapTracking`.
  _InternalState _internalState = _InternalState.off;

  final Mutex _m = Mutex();

  Timer? _lastPositionTooOldTimer;

  StreamSubscription<LocationData>? _locationUpdateSub;
  StreamSubscription<LocationError>? _locationErrorSub;

  // Notify the user that the recording was unexpectedly stopped on iOS.
  // On Android, this does not work, and we achive this by using foreground task.
  // On iOS we rely on this to make sure user will be notified when the app is
  // killed during recording.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

  GpsManager() {
    _locationService = GeoLocatorService();
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

  void _saveIsRecordingState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    prefs.setBool(
        isRecordingPrefsKey, recordingStatus == GpsRecordingStatus.recording);
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
      await PreferencesManager.setUnexpectedExitNotificationStatus(true);
      return;
    }

    var context = navigatorKey.currentState?.context;
    if (context != null && context.mounted) {
      await showCommonDialog(
          context,
          context.tr(
              "unexpected_exit_notification.notification_permission_reason"));
    }

    final result = await Permission.notification.request();

    if (result.isGranted) {
      await PreferencesManager.setUnexpectedExitNotificationStatus(true);
    } else {
      await PreferencesManager.setUnexpectedExitNotificationStatus(false);
    }
  }

  Future<void> _locationPermissionDeniedDialog() async {
    var context = navigatorKey.currentState?.context;
    if (context != null && context.mounted) {
      await showCommonDialog(
          context, context.tr("home.location_permission_denied"));
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
      await _locationPermissionDeniedDialog();
      await Geolocator.openAppSettings();
      throw "Please allow location permissions";
    }

    if (!await Permission.location.isGranted) {
      if (!await Permission.location.request().isGranted) {
        await _locationPermissionDeniedDialog();
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
        await _locationService.stopLocationUpdates();
        await _locationUpdateSub?.cancel();
        await _locationErrorSub?.cancel();
        _locationUpdateSub = null;
        _locationErrorSub = null;
        latestPosition = null;
        if (oldState == _InternalState.recording) {
          await _notificationWhenAppIsKilledPlugin
              .cancelNotificationOnKillService();
        }
      }

      // turnning on if needed
      if (newState != _InternalState.off) {
        log.info("[GpsManager] turning on gps stream. new state: $newState");
        bool enableBackground = newState == _InternalState.recording;
        await _locationService.startLocationUpdates(enableBackground);

        _locationUpdateSub = _locationService.onLocationUpdate((data) {
          latestPosition = data;
          notifyListeners();

          if (_internalState == _InternalState.recording) {
            api.onLocationUpdate(
              rawDataList: [
                RawData(
                  point: Point(
                    latitude: data.latitude,
                    longitude: data.longitude,
                  ),
                  timestampMs: data.timestampMs,
                  accuracy: data.accuracy,
                  altitude: data.altitude,
                  speed: data.speed,
                )
              ],
              receviedTimestampMs: DateTime.now().millisecondsSinceEpoch,
            );
          }
        });

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
            await PreferencesManager.getUnexpectedExitNotificationStatus()) {
          await _notificationWhenAppIsKilledPlugin.setNotificationOnKillService(
            ArgsForKillNotification(
                title: tr("unexpected_exit_notification.notification_title"),
                description:
                    tr("unexpected_exit_notification.notification_message"),
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
