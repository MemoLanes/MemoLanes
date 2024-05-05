import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:project_dv/gps_recording_state.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:provider/provider.dart';

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
    getRawDataMode().then((value) => setState(() {
          enabled = value;
        }));
  }

  @override
  Widget build(BuildContext context) {
    return Switch(
      value: enabled,
      activeColor: Colors.red,
      onChanged: (bool value) async {
        await toggleRawDataMode(enable: value);
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
    var gpsRecordingState = context.watch<GpsRecordingState>();
    var position = gpsRecordingState.latestPosition;
    var message = "";
    if (position != null) {
      message =
          '[${position.timestamp.toLocal()}]${position.latitude.toStringAsFixed(6)}, ${position.longitude.toStringAsFixed(6)} ~${position.accuracy.toStringAsFixed(1)}';
    }
    return Center(
      child: Column(
        children: [
          Text(message),
          Text(gpsRecordingState.isRecording ? "Recording" : "Idle"),
          ElevatedButton(
            onPressed: gpsRecordingState.toggle,
            child: Text(gpsRecordingState.isRecording ? "Stop" : "Start"),
          ),
          ElevatedButton(
            onPressed: () async {
              if (await finalizeOngoingJourney()) {
                Fluttertoast.showToast(msg: "New journey added");
              }
            },
            child: const Text("Start a new journey"),
          ),
          const Text("Raw data"),
          const RawDataSwitch(),
        ],
      ),
    );
  }
}
