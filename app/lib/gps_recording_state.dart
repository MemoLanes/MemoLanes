import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:mutex/mutex.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:async/async.dart';
import 'package:project_dv/src/rust/api/api.dart';

class AutoJourneyFinalizer {
  AutoJourneyFinalizer() {
    _start();
  }

  void _start() async {
    await tryOnce();
    Timer.periodic(const Duration(minutes: 5), (timer) async {
      await tryOnce();
    });
  }

  Future<void> tryOnce() async {
    if (await tryAutoFinalizeJourny()) {
      Fluttertoast.showToast(msg: "New journey added");
    }
  }
}

class GpsRecordingState extends ChangeNotifier {
  var isRecording = false;
  Position? latestPosition;

  LocationSettings? _locationSettings;
  StreamSubscription<Position>? _positionStream;
  final Mutex _m = Mutex();
  final AutoJourneyFinalizer _autoJourneyFinalizer = AutoJourneyFinalizer();

  RestartableTimer? restartableTimer;
  List<Position> dataList = [];

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
  }

  Future<void> _onLocationUpdate(List<Position> positionList) async {
    if (!isRecording || positionList.isEmpty) return;
    notifyListeners();
    positionList.sort((a, b) => a.timestamp.compareTo(b.timestamp));
    latestPosition = positionList.last;
    for (Position position in positionList) {
      await onLocationUpdate(
          latitude: position.latitude,
          longitude: position.longitude,
          timestampMs: position.timestamp.millisecondsSinceEpoch,
          accuracy: position.accuracy,
          altitude: position.altitude,
          speed: position.speed);
    }
  }

  bool addData(Position position) {
    dataList.add(position);
    restartableTimer ??= RestartableTimer(
        Duration(milliseconds: 200),
        () {
          readData();
        },
      );
    restartableTimer?.reset();
    return true;
  }

  void readData() {
    List<Position> tmpList = dataList;
    dataList = [];
    _onLocationUpdate(tmpList);
  }

  void toggle() async {
    await _m.protect(() async {
      await _autoJourneyFinalizer.tryOnce();
      if (isRecording) {
        await _positionStream?.cancel();
        restartableTimer?.cancel();
        readData();
        _positionStream = null;
        latestPosition = null;
      } else {
        try {
          if (!await Permission.notification.isGranted) {
            await Permission.notification.request();
            if (!await Permission.notification.isGranted) {
              throw "notification permission not granted";
            }
          }

          /// if GPS service is enabled
          bool serviceEnabled = await Geolocator.isLocationServiceEnabled();
          if (!serviceEnabled) {
            /// Location services are not enabled, ask the user to enable location services
            var res = await Geolocator.openLocationSettings();
            if (!res) {
              /// refused
              throw "Location services not enabled";
            }
          }

          /// Getting Permissions
          LocationPermission permission = await Geolocator.checkPermission();
          if (permission == LocationPermission.denied) {
            /// Previous access to device location denied, reapply permission
            permission = await Geolocator.requestPermission();
            if (permission == LocationPermission.denied ||
                permission == LocationPermission.deniedForever) {
              /// Rejected again
              throw "Location permission not granted";
            }
          } else if (permission == LocationPermission.deniedForever) {
            /// Previously permissions were permanently denied, open the app permissions settings page
            await Geolocator.openAppSettings();
            throw "Please allow location permissions";
          }
        } catch (e) {
          Fluttertoast.showToast(msg: e.toString());
          return;
        }
        _positionStream ??=
            Geolocator.getPositionStream(locationSettings: _locationSettings)
                .listen((Position? position) async {
          if (position != null) {
            await addData(position);
          }
        });
      }
      isRecording = !isRecording;
      notifyListeners();
    });
  }
}
