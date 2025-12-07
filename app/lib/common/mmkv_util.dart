import 'package:mmkv/mmkv.dart';

class MMKVKey {
  static const String isUnexpectedExitNotificationEnabled =
      'isUnexpectedExitNotificationEnabled';
  static const String dbOptimizationCheck = "dbOptimizationCheck";
  static const String mainMapState = "MainMap.mapState";
  static const String isRecording = "GpsManager.isRecording";
  static const String privacyAgreementAccepted = "privacyAgreementAccepted";
  static const String requestBatteryOptimization = 'requestBatteryOptimization';
  static const String requestNotification = 'requestNotification';
}

class MMKVUtil {
  static late MMKV _mmkv;

  static Future<void> init() async {
    await MMKV.initialize();
    _mmkv = MMKV.defaultMMKV();
  }

  /// put bool
  static bool putBool(String key, bool value, [int? expireDurationInSecond]) {
    return _mmkv.encodeBool(key, value, expireDurationInSecond);
  }

  /// get bool
  static bool getBool(String key, {bool defaultValue = false}) {
    return _mmkv.decodeBool(key, defaultValue: defaultValue);
  }

  /// put int
  static bool putInt(String key, int value) {
    return _mmkv.encodeInt(key, value);
  }

  /// get int
  static int getInt(String key, {int defaultValue = 0}) {
    return _mmkv.decodeInt(key, defaultValue: defaultValue);
  }

  /// put string
  static bool putString(String key, String? value) {
    return _mmkv.encodeString(key, value);
  }

  /// get string
  static String getString(String key, {String defaultValue = ''}) {
    return _mmkv.decodeString(key) ?? defaultValue;
  }

  /// remove key
  static void removeAppKey(String key) {
    return _mmkv.removeValue(key);
  }
}
