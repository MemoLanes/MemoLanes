import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/service/location/geolocator_service.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps_processor.dart';
import 'package:mutex/mutex.dart';
import 'package:notification_when_app_is_killed/model/args_for_ios.dart';
import 'package:notification_when_app_is_killed/model/args_for_kill_notification.dart';
import 'package:notification_when_app_is_killed/notification_when_app_is_killed.dart';
import 'package:permission_handler/permission_handler.dart';

enum GpsRecordingStatus { none, recording, paused }

// `recording` requires background location but `justForTracking` does not.
enum _InternalState { off, recording, justForTracking }

bool _positionTooOld(LocationData data, {int staleThresholdMs = 12 * 1000}) {
  final now = DateTime.now().millisecondsSinceEpoch;
  return now - data.timestampMs >= staleThresholdMs;
}

class GpsManager extends ChangeNotifier {
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

  // Notify the user that the recording was unexpectedly stopped.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

  // basically, we try to finalize every 30 mins + when there isn't a meaningful update in a while.
  DateTime? _tryFinalizeJourneyCountDown;

  // We only start listening to the location service after this.
  // Otherwise we may start it before the app is fully ready (e.g. i18n not ready).
  bool _fullyReady = false;

  GpsManager() {
    _locationService = GeoLocatorService();
    _initState();
  }

  LocationBackend get locationBackend => _locationService.locationBackend;

  void _initState() async {
    await _m.protect(() async {
      await _tryFinalizeJourneyWithoutLock();
      Timer.periodic(const Duration(minutes: 30), (timer) async {
        await _m.protect(() async {
          await _tryFinalizeJourneyWithoutLock();
        });
        _tryFinalizeJourneyCountDown = DateTime.now();
      });

      if (MMKVUtil.getBool(MMKVKey.isRecording) &&
          await PermissionService().checkLocationPermission()) {
        recordingStatus = GpsRecordingStatus.recording;
      } else if (await api.hasOngoingJourney()) {
        recordingStatus = GpsRecordingStatus.paused;
      }
      // notify record button
      notifyListeners();
    });
  }

  Future<void> _tryFinalizeJourneyWithoutLock() async {
    if (await api.tryAutoFinalizeJourney()) {
      Fluttertoast.showToast(msg: "New journey added");
      if (recordingStatus == GpsRecordingStatus.paused) {
        recordingStatus = GpsRecordingStatus.none;
        notifyListeners();
        await _syncInternalStateWithoutLock();
      }
    }
  }

  Future<void> _syncInternalStateWithoutLock() async {
    // do nothing until fully ready, we will sync it again when it becomes ready
    // for the first time.
    if (!_fullyReady) {
      return;
    }

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
        _locationUpdateSub = null;
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

        _locationUpdateSub = _locationService.onLocationUpdate((data) async {
          if (_positionTooOld(data)) {
            return;
          }
          latestPosition = data;
          notifyListeners();

          if (_internalState == _InternalState.recording) {
            var now = DateTime.now();

            var last = _tryFinalizeJourneyCountDown;
            if (last != null && now.difference(last).inSeconds >= 60) {
              await _m.protect(() async {
                await _tryFinalizeJourneyWithoutLock();
              });
              _tryFinalizeJourneyCountDown = now;
            }

            var meaningful = await api.onLocationUpdate(
              rawData: RawData(
                point: Point(
                  latitude: data.latitude,
                  longitude: data.longitude,
                ),
                timestampMs: data.timestampMs,
                accuracy: data.accuracy,
                altitude: data.altitude,
                speed: data.speed,
              ),
              receivedTimestampMs: now.millisecondsSinceEpoch,
            );

            if (meaningful) {
              _tryFinalizeJourneyCountDown = now;
            }
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

        final unexpectedExitNotificationStatus =
            await Permission.notification.isGranted &&
                MMKVUtil.getBool(MMKVKey.isUnexpectedExitNotificationEnabled,
                    defaultValue: true);
        if (newState == _InternalState.recording &&
            unexpectedExitNotificationStatus) {
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

  Future<void> changeRecordingState(GpsRecordingStatus to) async {
    if (to == GpsRecordingStatus.recording) {
      if (!await PermissionService().checkAndRequestPermission()) {
        return;
      }
    }

    await _m.protect(() async {
      var needToFinalize =
          recordingStatus != to && to == GpsRecordingStatus.none;
      recordingStatus = to;
      notifyListeners();

      await _syncInternalStateWithoutLock();
      MMKVUtil.putBool(
        MMKVKey.isRecording,
        recordingStatus == GpsRecordingStatus.recording,
      );

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
      if (!await PermissionService().checkAndRequestPermission()) {
        return;
      }
    }

    await _m.protect(() async {
      mapTracking = enable;
      await _syncInternalStateWithoutLock();
    });
  }

  void readyToStart() {
    _fullyReady = true;
    // sync internal state for the first time
    _m.protect(() async {
      await _syncInternalStateWithoutLock();
    });
  }
}
