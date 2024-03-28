import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:mutex/mutex.dart';
import 'package:permission_handler/permission_handler.dart';

import 'package:project_dv/src/rust/api/api.dart';

class GpsRecordingState extends ChangeNotifier {
  var isRecording = false;
  Position? latestPosition;

  LocationSettings? _locationSettings;
  StreamSubscription<Position>? _positionStream;
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

  void toggle() async {
    await _m.protect(() async {
      if (isRecording) {
        await _positionStream?.cancel();
        _positionStream = null;
        latestPosition = null;
      } else {
        if (!await Permission.notification.isGranted) {
          await Permission.notification.request();
          if (!await Permission.notification.isGranted) {
            Fluttertoast.showToast(msg: "notification permission not granted");
            throw Exception("notification permission not granted");
          }
        }

        /// if GPS service is enabled
        bool serviceEnabled = await Geolocator.isLocationServiceEnabled();
        if (!serviceEnabled) {
          /// Location services are not enabled, ask the user to enable location services
          var res = await Geolocator.openLocationSettings();
          if (!res) {
            /// refused
            Fluttertoast.showToast(msg: "Location services not enabled");
            throw Exception("Location services not enabled");
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
            Fluttertoast.showToast(msg: "Location permission not granted");
            throw Exception("Location permission not granted");
          }
        } else if (permission == LocationPermission.deniedForever) {
          /// Previously permissions were permanently denied, open the app permissions settings page
          Fluttertoast.showToast(msg: "Please allow location permissions");
          await Geolocator.openAppSettings();
          throw Exception("Location permission not granted");
        }
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
