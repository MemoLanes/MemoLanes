import 'package:flutter/material.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:provider/provider.dart';
import 'package:easy_localization/easy_localization.dart';

class GPSPage extends StatefulWidget {
  const GPSPage({super.key});

  @override
  State<GPSPage> createState() => _GPSPageState();
}

class _GPSPageState extends State<GPSPage> {
  // TODO: this should probably be some reusable pattern
  Future<void> _showEndJourneyDialog() async {
    return showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (BuildContext context) {
        return AlertDialog(
          backgroundColor: Colors.white,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(24),
          ),
          title: Text(
            context.tr('home.end_journey_title'),
            style: const TextStyle(color: Colors.black),
          ),
          content: Text(
            context.tr('home.end_journey_message'),
            style: const TextStyle(color: Colors.black54),
          ),
          actionsPadding: const EdgeInsets.fromLTRB(24, 0, 24, 16),
          actions: <Widget>[
            FilledButton(
              onPressed: () => Navigator.of(context).pop(),
              style: FilledButton.styleFrom(
                backgroundColor: const Color(0xFFB4EC51),
                foregroundColor: Colors.black,
              ),
              child: Text(context.tr('common.cancel')),
            ),
            FilledButton(
              onPressed: () {
                Navigator.of(context).pop();
                context
                    .read<GpsRecordingState>()
                    .changeState(GpsRecordingStatus.none);
              },
              style: FilledButton.styleFrom(
                backgroundColor: Colors.red,
                foregroundColor: Colors.white,
              ),
              child: Text(context.tr('common.end')),
            ),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    var gpsRecordingState = context.watch<GpsRecordingState>();

    Widget controls;
    if (gpsRecordingState.status == GpsRecordingStatus.none) {
      controls = Center(
        child: ElevatedButton(
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
          child: Text(
            context.tr("home.start_new_journey"),
            style: const TextStyle(
              color: Colors.black,
              fontWeight: FontWeight.w400,
              fontSize: 20,
            ),
          ),
        ),
      );
    } else if (gpsRecordingState.status == GpsRecordingStatus.recording) {
      controls = Center(
        child: Row(
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
                padding:
                    const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
              ),
              child: Text(
                context.tr("home.pause"),
                style: const TextStyle(
                  color: Color(0xFFB4EC51),
                  fontWeight: FontWeight.w400,
                  fontSize: 20,
                ),
              ),
            ),
            const SizedBox(width: 24),
            ElevatedButton(
              onPressed: _showEndJourneyDialog,
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
        ),
      );
    } else {
      controls = Center(
        child: Row(
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
                padding:
                    const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
              ),
              child: Text(
                context.tr("home.resume"),
                style: const TextStyle(
                  color: Color(0xFFB4EC51),
                  fontWeight: FontWeight.w400,
                  fontSize: 20,
                ),
              ),
            ),
            const SizedBox(width: 24),
            ElevatedButton(
              onPressed: _showEndJourneyDialog,
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
        ),
      );
    }

    return controls;
  }
}
