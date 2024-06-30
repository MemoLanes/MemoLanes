import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:provider/provider.dart';

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
        ],
      ),
    );
  }
}
