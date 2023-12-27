import 'dart:async';
import 'dart:developer';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'package:mutex/mutex.dart';
import 'package:share_plus/share_plus.dart';

import 'ffi.dart' if (dart.library.html) 'ffi_web.dart';

/// `PokeGeolocatorTask` is a hacky workround on Android.
/// The behvior we observe is that the position stream from geolocator will
/// randomly pauses so updates are delayed or missed even when holding the
/// wakelock. However, if there something request the location, even if it is in
/// another app, the stream will resume. So the hack is to poke the geolocator
/// frequently.
class PokeGeolocatorTask {
  // TODO: Test on iOS
  bool running = false;
  PokeGeolocatorTask();

  factory PokeGeolocatorTask.start() {
    var task = PokeGeolocatorTask();
    task.running = true;
    if (defaultTargetPlatform == TargetPlatform.android) {
      task._loop();
    }
    return task;
  }

  _loop() async {
    await Future.delayed(const Duration(seconds: 5));
    // we don't care about the result
    if (running) {
      print("XXX");
      await Geolocator.getCurrentPosition(
              timeLimit: const Duration(seconds: 10))
          .then((_) => null)
          .catchError((_) => null);
      _loop();
    }
  }

  cancel() {
    running = false;
  }
}

class MainState extends ChangeNotifier {
  var initializing = true;
  var isRecording = false;
  LocationSettings? locationSettings;
  StreamSubscription<Position>? positionStream;
  PokeGeolocatorTask? pokeGeolocatorTask;
  var message = "";
  Mutex m = Mutex();

  init() {
    var result = () async {
      // TODO: handle all cases
      var permissionStatus = await Permission.locationAlways.request();
      log("permissionStatus: $permissionStatus");

      var accuracy = LocationAccuracy.best;
      var distanceFilter = 0;
      if (defaultTargetPlatform == TargetPlatform.android) {
        locationSettings = AndroidSettings(
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
        locationSettings = AppleSettings(
          accuracy: accuracy,
          activityType: ActivityType.fitness,
          distanceFilter: distanceFilter,
          pauseLocationUpdatesAutomatically: true,
          showBackgroundLocationIndicator: false,
        );
      } else {
        locationSettings = LocationSettings(
          accuracy: accuracy,
          distanceFilter: distanceFilter,
        );
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
        if (positionStream == null) {
          pokeGeolocatorTask ??= PokeGeolocatorTask.start();
          positionStream =
              Geolocator.getPositionStream(locationSettings: locationSettings)
                  .listen((Position? position) async {
            if (!isRecording) return;
            if (position == null) return;
            message =
                ('[${position.timestamp.toLocal()}]${position.latitude.toString()}, ${position.longitude.toString()} ${position.altitude.toString()} ~${position.accuracy.toString()}');
            notifyListeners();

            print("YYY: ${position.timestamp}");

            var latitude = position.latitude;
            var longitude = position.longitude;
            var accuracy = position.accuracy;
            await api.onLocationUpdate(
                latitude: latitude,
                longitude: longitude,
                timestampMs: position.timestamp.millisecondsSinceEpoch,
                accuracy: accuracy,
                altitude: position.altitude,
                speed: position.speed);
          });
        }
      } else {
        await positionStream?.cancel();
        positionStream = null;
        pokeGeolocatorTask?.cancel();
        pokeGeolocatorTask = null;
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
