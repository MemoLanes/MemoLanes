import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:path_provider/path_provider.dart';
import 'package:mutex/mutex.dart';
import 'package:share_plus/share_plus.dart';
import 'dart:io';
import 'package:location/location.dart';

class MainState extends ChangeNotifier {
  var initializing = true;
  var isRecording = false;
  var message = "";
  var location = Location();
  StreamSubscription<LocationData>? locationSubscription;
  Mutex m = Mutex();
  IOSink? dataCsvFileSink;

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
          if (dataCsvFileSink == null) {
            final directory = await getApplicationDocumentsDirectory();
            final path = directory.path;
            dataCsvFileSink = File('$path/data.csv')
                .openWrite(mode: FileMode.writeOnlyAppend);
          }
          // TODO: null?
          var timestamp = locationData.time!.toInt();
          var now = DateTime.fromMillisecondsSinceEpoch(timestamp);
          message =
              ('[${now.toLocal()}]${locationData.latitude.toString()}, ${locationData.longitude.toString()} ${locationData.altitude.toString()} ~${locationData.accuracy.toString()}');
          notifyListeners();

          dataCsvFileSink?.writeln(
              '${now.toLocal()},${timestamp.toString()},${locationData.latitude.toString()},${locationData.longitude.toString()},${locationData.altitude.toString()},${locationData.accuracy.toString()}');
        });
      } else {
        location.enableBackgroundMode(enable: false);
        await locationSubscription?.cancel();

        message = "";
        await dataCsvFileSink?.flush();
        await dataCsvFileSink?.close();
        dataCsvFileSink = null;
      }
      notifyListeners();
    });
  }

  void clearFile() async {
    await m.protect(() async {
      if (!isRecording) {
        final directory = await getApplicationDocumentsDirectory();
        final path = directory.path;
        // override the old file
        var file = File('$path/data.csv');
        if (await file.exists()) {
          await file.delete();
        }
      }
    });
  }

  void exportFile() async {
    await m.protect(() async {
      if (!isRecording) {
        final directory = await getApplicationDocumentsDirectory();
        final path = directory.path;
        var file = File('$path/data.csv');
        if (await file.exists()) {
          Share.shareXFiles([XFile('$path/data.csv')]);
        }
      }
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
          ElevatedButton(
            onPressed: mainState.isRecording ? null : mainState.clearFile,
            child: const Text("Clear File"),
          ),
          ElevatedButton(
            onPressed: mainState.isRecording ? null : mainState.exportFile,
            child: const Text("Export File"),
          ),
        ],
      ),
    );
  }
}
