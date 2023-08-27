import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:mutex/mutex.dart';
import 'package:location/location.dart';

import 'ffi.dart' if (dart.library.html) 'ffi_web.dart';

class MainState extends ChangeNotifier {
  var initializing = true;
  var isRecording = false;
  var message = "";
  var location = Location();
  StreamSubscription<LocationData>? locationSubscription;
  Mutex m = Mutex();

  init() {
    var result = () async {
      bool serviceEnabled;
      PermissionStatus permissionGranted;

      serviceEnabled = await location.serviceEnabled();
      if (!serviceEnabled) {
        serviceEnabled = await location.requestService();
        if (!serviceEnabled) {
          return Future.error('Location services are disabled.');
        }
      }

      permissionGranted = await location.hasPermission();
      if (permissionGranted == PermissionStatus.denied) {
        permissionGranted = await location.requestPermission();
        if (permissionGranted != PermissionStatus.granted) {
          return Future.error('Location permissions are denied');
        }
      }

      initializing = false;
      notifyListeners();
      return;
    }();

    result.then((void _) {
      return;
    }, onError: (error) => print("Error : $error"));
    return;
  }

  void toggle() async {
    await m.protect(() async {
      isRecording = !isRecording;
      if (isRecording) {
        await location.enableBackgroundMode(enable: true);
        locationSubscription = location.onLocationChanged
            .listen((LocationData locationData) async {
          var timestamp = locationData.time?.toInt();
          if (timestamp == null) return;
          var now = DateTime.fromMillisecondsSinceEpoch(timestamp);
          message =
              ('[${now.toLocal()}]${locationData.latitude.toString()}, ${locationData.longitude.toString()} ${locationData.altitude.toString()} ~${locationData.accuracy.toString()}');
          notifyListeners();

          var latitude = locationData.latitude;
          var longitude = locationData.longitude;
          var timestampMs = locationData.time?.toInt();
          var accuracy = locationData.accuracy;
          if (latitude != null &&
              longitude != null &&
              timestampMs != null &&
              accuracy != null) {
            await api.onLocationUpdate(
                latitude: latitude,
                longitude: longitude,
                timestampMs: timestampMs,
                accuracy: accuracy,
                altitude: locationData.altitude,
                speed: locationData.speed);
          }
        });
      } else {
        location.enableBackgroundMode(enable: false);
        await locationSubscription?.cancel();

        message = "";
      }
      notifyListeners();
    });
  }
}

class GPS extends StatelessWidget {
  const GPS({super.key});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider(
        create: (context) {
          var mainState = MainState();
          mainState.init();
          return mainState;
        },
        child: const GPSPage());
  }
}

class GPSPage extends StatelessWidget {
  const GPSPage({super.key});

  @override
  Widget build(BuildContext context) {
    var mainState = context.watch<MainState>();

    return Center(
      child: Column(
        children: [
          Text(mainState.message),
          Text(mainState.isRecording ? "Recording" : "Idle"),
          ElevatedButton(
            onPressed: mainState.initializing ? null : mainState.toggle,
            child: Text(mainState.isRecording ? "Stop" : "Start"),
          ),
        ],
      ),
    );
  }
}
