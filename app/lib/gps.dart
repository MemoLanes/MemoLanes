import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:mutex/mutex.dart';
import 'package:location/location.dart';
import 'package:share_plus/share_plus.dart';

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

class ExportRawData extends StatefulWidget {
  const ExportRawData({super.key});

  @override
  _ExportRawDataState createState() => _ExportRawDataState();
}

class _ExportRawDataState extends State<ExportRawData> {
  void _showDialog(List<RawDataFile> items) async {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text('Select an item'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: items.map((item) {
                return ListTile(
                  title: Text(item.name),
                  onTap: () {
                    Share.shareXFiles([XFile(item.path)]);
                    Navigator.of(context).pop();
                  },
                );
              }).toList(),
            ),
          ),
          actions: <Widget>[
            TextButton(
              child: const Text('Cancel'),
              onPressed: () {
                Navigator.of(context).pop();
              },
            ),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: () async {
        var items = await api.listAllRawData();
        _showDialog(items);
      },
      child: const Text('Export'),
    );
  }
}

class RawDataSwitch extends StatefulWidget {
  const RawDataSwitch({super.key});

  @override
  State<RawDataSwitch> createState() => _RawDataSwitchState();
}

class _RawDataSwitchState extends State<RawDataSwitch> {
  bool enabled = false;

  @override
  initState() {
    super.initState();
    api.getRawDataMode().then((value) => setState(() {
          enabled = value;
        }));
  }

  @override
  Widget build(BuildContext context) {
    return Switch(
      value: enabled,
      activeColor: Colors.red,
      onChanged: (bool value) async {
        await api.toggleRawDataMode(enable: value);
        setState(() {
          enabled = value;
        });
      },
    );
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
            onPressed: () async {
              await api.finalizeOngoingJourney();
            },
            child: const Text("Start a new journey"),
          ),
          const Text("Raw data"),
          const RawDataSwitch(),
          const ExportRawData(),
        ],
      ),
    );
  }
}
