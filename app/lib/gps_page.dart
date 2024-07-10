import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:provider/provider.dart';
import 'package:memolanes/extensions/l10n_context.dart';

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
          Text(gpsRecordingState.isRecording
              ? context.l10n.recording
              : context.l10n.idle),
          ElevatedButton(
            onPressed: gpsRecordingState.toggle,
            child: Text(gpsRecordingState.isRecording
                ? context.l10n.stop
                : context.l10n.start),
          ),
          ElevatedButton(
            onPressed: () async {
              if (await finalizeOngoingJourney()) {
                Fluttertoast.showToast(msg: "New journey added");
              }
            },
            child: Text(context.l10n.startNewJourney),
          ),
        ],
      ),
    );
  }
}
