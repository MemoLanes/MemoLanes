import 'package:flutter/material.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:provider/provider.dart';
import 'package:memolanes/extensions/l10n_context.dart';

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
              ? context.l10n.gpsRecording
              : context.l10n.gpsIdle),
          Builder(
            builder: (BuildContext context) {
              if (gpsRecordingState.status == GpsRecordingStatus.none) {
                return ElevatedButton(
                    onPressed: () async {
                      gpsRecordingState
                          .changeState(GpsRecordingStatus.recording);
                    },
                    child: Text(context.l10n.gpsStartNewJourney));
              } else if (gpsRecordingState.status ==
                  GpsRecordingStatus.recording) {
                return Column(children: [
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState
                            .changeState(GpsRecordingStatus.paused);
                      },
                      child: Text(context.l10n.gpsPause)),
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState.changeState(GpsRecordingStatus.none);
                      },
                      child: Text(context.l10n.gpsStop)),
                ]);
              } else if (gpsRecordingState.status ==
                  GpsRecordingStatus.paused) {
                return Column(children: [
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState
                            .changeState(GpsRecordingStatus.recording);
                      },
                      child: Text(context.l10n.gpsRusme)),
                  ElevatedButton(
                      onPressed: () async {
                        gpsRecordingState.changeState(GpsRecordingStatus.none);
                      },
                      child: Text(context.l10n.gpsStop)),
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
