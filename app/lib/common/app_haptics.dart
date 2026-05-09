import 'dart:async';

import 'package:haptic_feedback/haptic_feedback.dart';
import 'package:memolanes/common/mmkv_util.dart';

abstract final class AppHaptics {
  AppHaptics._();

  static const HapticsUsage _defaultUsage = HapticsUsage.touch;
  static const bool _defaultUseAndroidHapticConstants = true;

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

  /// Single entry to [Haptics.vibrate]. Typed helpers forward here; pass [usage] /
  /// [useAndroidHapticConstants] to override app defaults.
  static void vibrate(
    HapticsType type, {
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) {
    if (!isUserHapticsEnabled) return;
    unawaited(
      Haptics.vibrate(
        type,
        usage: usage ?? _defaultUsage,
        useAndroidHapticConstants:
            useAndroidHapticConstants ?? _defaultUseAndroidHapticConstants,
      ),
    );
  }

  static void success({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.success,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void warning({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.warning,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void error({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.error,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void light({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.light,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void medium({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.medium,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void heavy({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.heavy,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void rigid({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.rigid,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void soft({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.soft,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);

  static void selection({
    HapticsUsage? usage,
    bool? useAndroidHapticConstants,
  }) =>
      vibrate(HapticsType.selection,
          usage: usage, useAndroidHapticConstants: useAndroidHapticConstants);
}
