import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:memolanes/utils.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

class RecordingButtons extends StatefulWidget {
  const RecordingButtons({super.key});

  @override
  State<RecordingButtons> createState() => _RecordingButtonsState();
}

class _RecordingButtonsState extends State<RecordingButtons> {
  Future<void> _showEndJourneyDialog() async {
    final gpsManager = context.read<GpsManager>();
    final shouldEndJourney = await showCommonDialog(
        context, context.tr("home.end_journey_message"),
        hasCancel: true,
        title: context.tr("home.end_journey_title"),
        confirmButtonText: context.tr("common.end"),
        confirmGroundColor: Colors.red,
        confirmTextColor: Colors.white);

    if (shouldEndJourney) {
      gpsManager.changeRecordingState(GpsRecordingStatus.none);
    }
  }

  @override
  Widget build(BuildContext context) {
    var gpsManager = context.watch<GpsManager>();

    Widget controls;
    if (gpsManager.recordingStatus == GpsRecordingStatus.none) {
      controls = Center(
        child: PointerInterceptor(
            child: ElevatedButton(
          onPressed: () async {
            gpsManager.changeRecordingState(GpsRecordingStatus.recording);
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
        )),
      );
    } else if (gpsManager.recordingStatus == GpsRecordingStatus.recording) {
      controls = Center(
        child: Stack(
          children: [
            // 中间按钮（大）
            Align(
              alignment: Alignment.center,
              child: PointerInterceptor(
                child: ElevatedButton(
                  onPressed: () async {
                    gpsManager.changeRecordingState(GpsRecordingStatus.paused);
                  },
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.black,
                    shape: const CircleBorder(),
                    padding: const EdgeInsets.all(16),
                  ),
                  child: Icon(
                    Icons.pause,
                    color: Colors.white,
                    size: 50,
                  ),
                ),
              ),
            ),
            // 左边按钮（小），通过内层 Align + 外层对齐占位
            Align(
              alignment: const Alignment(-1, 0), // 左半居中
              child: SizedBox(
                width: 80, // 对齐参考用，占位，但不影响按钮大小
                height: 80,
                child: Center(
                  child: PointerInterceptor(
                    child: ElevatedButton(
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
                  ),
                ),
              ),
            ),
          ],
        ),
      );
    } else {
      controls = controls = Center(
        child: Stack(
          children: [
            Align(
              alignment: Alignment.center,
              child: PointerInterceptor(
                  child: ElevatedButton(
                onPressed: () async {
                  gpsManager.changeRecordingState(GpsRecordingStatus.recording);
                },
                style: ElevatedButton.styleFrom(
                  backgroundColor: const Color(0xFFB4EC51),
                  shape: const CircleBorder(),
                  padding: const EdgeInsets.all(16),
                ),
                child: Icon(
                  Icons.play_arrow,
                  color: Colors.black,
                  size: 50,
                ),
              )),
            ),
            Align(
              alignment: const Alignment(-1, 0),
              child: SizedBox(
                width: 80,
                height: 80,
                child: Center(
                  child: PointerInterceptor(
                      child: ElevatedButton(
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
                  )),
                ),
              ),
            ),
          ],
        ),
      );
    }

    return controls;
  }
}
