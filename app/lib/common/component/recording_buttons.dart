import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/animation/ink_button.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/utils.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

class RecordingButtons extends StatefulWidget {
  const RecordingButtons({super.key});

  @override
  State<RecordingButtons> createState() => _RecordingButtonsState();
}

class _RecordingButtonsState extends State<RecordingButtons> {
  static const _pressFeedbackDuration = Duration(milliseconds: 120);
  static const _switchDuration = Duration(milliseconds: 220);
  bool _pendingAction = false;

  Future<void> _runAfterPressFeedback(Future<void> Function() action) async {
    if (_pendingAction) return;

    _pendingAction = true;
    await Future.delayed(_pressFeedbackDuration);
    if (!mounted) return;

    try {
      await action();
    } finally {
      _pendingAction = false;
    }
  }

  Future<void> _showEndJourneyDialog() async {
    AppHaptics.warning();
    await _runAfterPressFeedback(() async {
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
    });
  }

  Future<void> _changeRecordingStateAfterPress(
    GpsManager gpsManager,
    GpsRecordingStatus status,
  ) async {
    await _runAfterPressFeedback(() async {
      gpsManager.changeRecordingState(status);
    });
  }

  @override
  Widget build(BuildContext context) {
    var gpsManager = context.watch<GpsManager>();
    final recordingStatus = gpsManager.recordingStatus;

    Widget controls;
    if (recordingStatus == GpsRecordingStatus.none) {
      controls = Center(
        child: PointerInterceptor(
          child: InkButton.pill(
            backgroundColor: const Color(0xFFB4EC51),
            overlayColor: Colors.black.withValues(alpha: 0.16),
            onPressed: () async {
              AppHaptics.heavy();
              await _changeRecordingStateAfterPress(
                gpsManager,
                GpsRecordingStatus.recording,
              );
            },
            child: Text(
              context.tr("home.start_new_journey"),
              style: const TextStyle(
                color: Colors.black,
                fontWeight: FontWeight.w400,
                fontSize: 20,
              ),
            ),
          ),
        ),
      );
    } else if (recordingStatus == GpsRecordingStatus.recording) {
      controls = Center(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            PointerInterceptor(
              child: InkButton.pill(
                backgroundColor: Colors.black,
                overlayColor: Colors.white.withValues(alpha: 0.18),
                onPressed: () async {
                  AppHaptics.medium();
                  await _changeRecordingStateAfterPress(
                    gpsManager,
                    GpsRecordingStatus.paused,
                  );
                },
                child: Text(
                  context.tr("home.pause"),
                  style: const TextStyle(
                    color: Color(0xFFB4EC51),
                    fontWeight: FontWeight.w400,
                    fontSize: 20,
                  ),
                ),
              ),
            ),
            const SizedBox(width: 24),
            PointerInterceptor(
              child: InkButton.circle(
                backgroundColor: Colors.black,
                overlayColor: Colors.white.withValues(alpha: 0.18),
                size: 56,
                onPressed: _showEndJourneyDialog,
                child: const Icon(
                  Icons.close,
                  color: Colors.white,
                  size: 24,
                ),
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
            PointerInterceptor(
              child: InkButton.pill(
                backgroundColor: const Color(0xFFB4EC51),
                overlayColor: Colors.black.withValues(alpha: 0.16),
                onPressed: () async {
                  AppHaptics.medium();
                  await _changeRecordingStateAfterPress(
                    gpsManager,
                    GpsRecordingStatus.recording,
                  );
                },
                child: Text(
                  context.tr("home.resume"),
                  style: const TextStyle(
                    color: Colors.black,
                    fontWeight: FontWeight.w400,
                    fontSize: 20,
                  ),
                ),
              ),
            ),
            const SizedBox(width: 24),
            PointerInterceptor(
              child: InkButton.circle(
                backgroundColor: Colors.black,
                overlayColor: Colors.white.withValues(alpha: 0.18),
                size: 56,
                onPressed: _showEndJourneyDialog,
                child: const Icon(
                  Icons.close,
                  color: Colors.white,
                  size: 24,
                ),
              ),
            ),
          ],
        ),
      );
    }

    return SizedBox(
      width: double.infinity,
      child: AnimatedSwitcher(
        duration: _switchDuration,
        reverseDuration: _switchDuration,
        switchInCurve: Curves.easeOutCubic,
        switchOutCurve: Curves.easeInCubic,
        layoutBuilder: (currentChild, previousChildren) {
          return Stack(
            alignment: Alignment.center,
            children: [
              ...previousChildren,
              if (currentChild != null) currentChild,
            ],
          );
        },
        transitionBuilder: (child, animation) {
          return FadeTransition(
            opacity: animation,
            child: child,
          );
        },
        child: KeyedSubtree(
          key: ValueKey(recordingStatus),
          child: controls,
        ),
      ),
    );
  }
}
