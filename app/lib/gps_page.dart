import 'package:flutter/material.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:provider/provider.dart';

class GPSPage extends StatefulWidget {
  const GPSPage({super.key});

  @override
  State<GPSPage> createState() => _GPSPageState();
}

class _GPSPageState extends State<GPSPage> {
  @override
  Widget build(BuildContext context) {
    var gpsRecordingState = context.watch<GpsRecordingState>();

    Widget controls;
    if (gpsRecordingState.status == GpsRecordingStatus.none) {
      controls = ElevatedButton(
        onPressed: () async {
          gpsRecordingState.changeState(GpsRecordingStatus.recording);
        },
        style: ElevatedButton.styleFrom(
          backgroundColor: const Color(0xFFB4EC51),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(9999),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
        ),
        child: const Text(
          "New Journey",
          style: TextStyle(
            color: Colors.black,
            fontWeight: FontWeight.w400,
            fontSize: 20,
          ),
        ),
      );
    } else if (gpsRecordingState.status == GpsRecordingStatus.recording) {
      controls = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          ElevatedButton(
            onPressed: () async {
              gpsRecordingState.changeState(GpsRecordingStatus.paused);
            },
            style: ElevatedButton.styleFrom(
              backgroundColor: Colors.black,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(9999),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
            ),
            child: const Text(
              "Pause",
              style: TextStyle(
                color: Color(0xFFB4EC51),
                fontWeight: FontWeight.w400,
                fontSize: 20,
              ),
            ),
          ),
          const SizedBox(width: 24),
          ElevatedButton(
            onPressed: () async {
              gpsRecordingState.changeState(GpsRecordingStatus.none);
            },
            style: ElevatedButton.styleFrom(
              backgroundColor: Colors.black,
              shape: const CircleBorder(),
              padding: const EdgeInsets.all(16),
            ),
            child: const Icon(
              Icons.close,
              color: Colors.white,
              size: 24,
            ),
          ),
        ],
      );
    } else {
      controls = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          ElevatedButton(
            onPressed: () async {
              gpsRecordingState.changeState(GpsRecordingStatus.recording);
            },
            style: ElevatedButton.styleFrom(
              backgroundColor: Colors.black,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(9999),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
            ),
            child: const Text(
              "Continue",
              style: TextStyle(
                color: Color(0xFFB4EC51),
                fontWeight: FontWeight.w400,
                fontSize: 20,
              ),
            ),
          ),
          const SizedBox(width: 24),
          ElevatedButton(
            onPressed: () async {
              gpsRecordingState.changeState(GpsRecordingStatus.none);
            },
            style: ElevatedButton.styleFrom(
              backgroundColor: Colors.black,
              shape: const CircleBorder(),
              padding: const EdgeInsets.all(16),
            ),
            child: const Icon(
              Icons.close,
              color: Colors.white,
              size: 24,
            ),
          ),
        ],
      );
    }

    return Center(child: controls);
  }
}
