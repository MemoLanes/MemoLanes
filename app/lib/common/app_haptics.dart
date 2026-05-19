import 'dart:async';

import 'package:haptic_feedback/haptic_feedback.dart';
import 'package:memolanes/common/mmkv_util.dart';

abstract final class AppHaptics {
  AppHaptics._();

  static const HapticsUsage _defaultUsage = HapticsUsage.touch;

  static bool? _userHapticsEnabled;

  static bool get isUserHapticsEnabled {
    _userHapticsEnabled ??=
        MMKVUtil.getBool(MMKVKey.hapticsFeedbackEnabled, defaultValue: true);
    return _userHapticsEnabled!;
  }

  static void setUserHapticsEnabled(bool enabled) {
    MMKVUtil.putBool(MMKVKey.hapticsFeedbackEnabled, enabled);
    _userHapticsEnabled = enabled;
  }

  /// Single entry to [Haptics.vibrate]. Typed helpers forward here; pass [usage]
  /// to override app defaults.
  static void vibrate(
    HapticsType type, {
    HapticsUsage? usage,
  }) {
    if (!isUserHapticsEnabled) return;
    unawaited(
      Haptics.vibrate(type,
          usage: usage ?? _defaultUsage,
          useAndroidHapticConstants:
              /* The behavior simulated by [false] is poor. */
              true),
    );
  }

  static void success({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.success, usage: usage);

  static void warning({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.warning, usage: usage);

  static void error({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.error, usage: usage);

  static void light({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.light, usage: usage);

  static void medium({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.medium, usage: usage);

  static void heavy({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.heavy, usage: usage);

  static void rigid({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.rigid, usage: usage);

  static void soft({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.soft, usage: usage);

  static void selection({
    HapticsUsage? usage,
  }) =>
      vibrate(HapticsType.selection, usage: usage);
}
