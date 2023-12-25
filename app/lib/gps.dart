import 'package:flutter/material.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'package:mutex/mutex.dart';
import 'package:background_location/background_location.dart';
import 'package:share_plus/share_plus.dart';

import 'ffi.dart' if (dart.library.html) 'ffi_web.dart';

class MainState extends ChangeNotifier {
  var initializing = true;
  var isRecording = false;
  var message = "";
  Mutex m = Mutex();

  init() {
    var result = () async {
      // TODO: handle all cases
      await Permission.locationAlways.request();

      isRecording = false;
      await BackgroundLocation.stopLocationService();

      await BackgroundLocation.setAndroidNotification(
        title: "Notification title",
        message: "Notification message",
        icon: "@mipmap/ic_launcher",
      );
      // 1 update/sec seems to be a reasonable value. I believe Guru Map is also
      // using this value.
      await BackgroundLocation.setAndroidConfiguration(1000);
      await BackgroundLocation.startLocationService(distanceFilter: 0);

      // TODO: not yet tested on iOS
      BackgroundLocation.getLocationUpdates((location) async {
        if (!isRecording) return;
        var timestamp = location.time?.toInt();
        if (timestamp == null) return;
        var time = DateTime.fromMillisecondsSinceEpoch(timestamp);
        message =
            ('[${time.toLocal()}]${location.latitude.toString()}, ${location.longitude.toString()} ${location.altitude.toString()} ~${location.accuracy.toString()}');
        notifyListeners();

        var latitude = location.latitude;
        var longitude = location.longitude;
        var accuracy = location.accuracy;
        if (latitude != null && longitude != null && accuracy != null) {
          await api.onLocationUpdate(
              latitude: latitude,
              longitude: longitude,
              timestampMs: timestamp,
              accuracy: accuracy,
              altitude: location.altitude,
              speed: location.speed);
        }
      });

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
        //To ensure that previously started services have been stopped, if desired
        await BackgroundLocation.stopLocationService();
        await BackgroundLocation.startLocationService();
      } else {
        await BackgroundLocation.stopLocationService();
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
