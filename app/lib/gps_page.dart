import 'package:flutter/material.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:provider/provider.dart';

class GPSPage extends StatelessWidget {
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
          Text(gpsRecordingState.status == GpsRecordingStatus.recording
              ? "Recording"
              : "Idle"),
          Builder(
            builder: (BuildContext context) {
              if (gpsRecordingState.status == GpsRecordingStatus.none) {
                return ElevatedButton(
                    onPressed: () async {
                      gpsRecordingState
                          .changeState(GpsRecordingStatus.recording);
                    },
                    child: const Text("Start new journey"));
              } else if (gpsRecordingState.status ==
                  GpsRecordingStatus.recording) {
                return Column(children: [
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState
                            .changeState(GpsRecordingStatus.paused);
                      },
                      child: const Text("Pause")),
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState.changeState(GpsRecordingStatus.none);
                      },
                      child: const Text("Stop")),
                ]);
              } else if (gpsRecordingState.status ==
                  GpsRecordingStatus.paused) {
                return Column(children: [
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState
                            .changeState(GpsRecordingStatus.recording);
                      },
                      child: const Text("Resume")),
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState.changeState(GpsRecordingStatus.none);
                      },
                      child: const Text("Stop")),
                ]);
              }
              // This is actually dead code
              return Container();
            },
          )
        ],
      ),
    );
  }
}
