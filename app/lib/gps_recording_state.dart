import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:geolocator/geolocator.dart';
import 'package:mutex/mutex.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:async/async.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:project_dv/src/rust/gps_processor.dart';

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
  }

  void _onPositionUpdate(Position position) {
    if (!isRecording) return;
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

    await onLocationUpdate(
        rawDataList: rawDataList, receviedTimestampMs: receviedTimestampMs);
  }

  void toggle() async {
    await _m.protect(() async {
      await _autoJourneyFinalizer.tryOnce();
      if (isRecording) {
        await _positionStream?.cancel();
        _positionStream = null;
        latestPosition = null;
        _positionBufferFlushTimer?.cancel();
        _positionBufferFlushTimer = null;
        await _flushPositionBuffer();
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
            _onPositionUpdate(position);
          }
        });
      }
      isRecording = !isRecording;
      notifyListeners();
    });
  }
}
