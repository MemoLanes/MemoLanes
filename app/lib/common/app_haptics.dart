import 'dart:async';

import 'package:haptic_feedback/haptic_feedback.dart';

abstract final class AppHaptics {
  AppHaptics._();

  /// Selection tick: ruler steps, toggles, single-select rows. Android uses CLOCK_TICK.
  static void selection() {
    unawaited(
      Haptics.vibrate(
        HapticsType.selection,
        usage: HapticsUsage.touch,
        useAndroidHapticConstants: true,
      ),
    );
  }

  /// Medium impact (e.g. primary actions). Maps to [HapticsType.medium].
  static void mediumImpact() {
    unawaited(
      Haptics.vibrate(
        HapticsType.medium,
        usage: HapticsUsage.touch,
        useAndroidHapticConstants: true,
      ),
    );
  }
}
