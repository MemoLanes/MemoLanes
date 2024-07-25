import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

class GPSPage extends StatefulWidget {
  const GPSPage({super.key});
  @override
  State<GPSPage> createState() => _GPSPageStatePage();
}

class _GPSPageStatePage extends State<GPSPage> {
  bool isNotStarted = true;

  @override
  void initState() {
    super.initState();
    _loadState();
  }

  Future<void> _loadState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    bool? startState = prefs.getBool('isNotStarted');
    if (startState != null && startState != isNotStarted) {
      setState(() {
        isNotStarted = startState;
      });
    }
  }

  Future<void> _saveState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    await prefs.setBool('isNotStarted', isNotStarted);
  }

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
              onPressed: () async {
                setState(() {
                  if (isNotStarted) {
                    isNotStarted = false;
                    if (!gpsRecordingState.isRecording) {
                      gpsRecordingState.toggle();
                    }
                  } else {
                    gpsRecordingState.toggle();
                  }
                });

                await _saveState();
              },
              child: Text(isNotStarted
                  ? "Start"
                  : (gpsRecordingState.isRecording ? "Pause" : "Continue"))),
          if (!isNotStarted)
            ElevatedButton(
              onPressed: () async {
                setState(() {
                  isNotStarted = true;
                });
                if (gpsRecordingState.isRecording) {
                  gpsRecordingState.toggle();
                }
                await _saveState();
                bool result = await finalizeOngoingJourney();
                if (result) {
                  Fluttertoast.showToast(msg: "New journey added");
                } else {
                  Fluttertoast.showToast(msg: "No journey detected");
                }
              },
              child: const Text("Stop"),
            )
          // ElevatedButton(
          //   onPressed: gpsRecordingState.toggle,
          //   child: Text(gpsRecordingState.isRecording ? "Stop" : "Start"),
          // ),
          // ElevatedButton(
          //   onPressed: () async {
          //     if (await finalizeOngoingJourney()) {
          //       Fluttertoast.showToast(msg: "New journey added");
          //     }
          //   },
          //   child: const Text("Start a new journey"),
          // ),
        ],
      ),
    );
  }
}
