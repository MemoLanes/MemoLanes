import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';

class PermissionPrefs {
  PermissionPrefs._privateConstructor();

  static final PermissionPrefs _instance =
      PermissionPrefs._privateConstructor();

  factory PermissionPrefs() => _instance;

  bool getPermissionSheetShown({
    bool logRead = false,
    String reason = 'read',
  }) {
    return _getBool(
      MMKVKey.permissionSheetShown,
      defaultValue: false,
      logRead: logRead,
      reason: reason,
    );
  }

  bool setPermissionSheetShown(
    bool value, {
    required String reason,
  }) {
    return _putBool(
      MMKVKey.permissionSheetShown,
      value,
      reason: reason,
    );
  }

  bool getRequestedLocation({
    bool logRead = false,
    String reason = 'read',
  }) {
    return _getBool(
      MMKVKey.requestedLocation,
      defaultValue: false,
      logRead: logRead,
      reason: reason,
    );
  }

  bool setRequestedLocation(
    bool value, {
    required String reason,
  }) {
    return _putBool(
      MMKVKey.requestedLocation,
      value,
      reason: reason,
    );
  }

  bool getRequestedBatteryOptimization({
    bool logRead = false,
    String reason = 'read',
  }) {
    return _getBool(
      MMKVKey.requestedBatteryOptimization,
      defaultValue: false,
      logRead: logRead,
      reason: reason,
    );
  }

  bool setRequestedBatteryOptimization(
    bool value, {
    required String reason,
  }) {
    return _putBool(
      MMKVKey.requestedBatteryOptimization,
      value,
      reason: reason,
    );
  }

  bool getRequestedNotification({
    bool logRead = false,
    String reason = 'read',
  }) {
    return _getBool(
      MMKVKey.requestedNotification,
      defaultValue: false,
      logRead: logRead,
      reason: reason,
    );
  }

  bool setRequestedNotification(
    bool value, {
    required String reason,
  }) {
    return _putBool(
      MMKVKey.requestedNotification,
      value,
      reason: reason,
    );
  }

  bool getUnexpectedExitNotificationEnabled({
    bool defaultValue = false,
    bool logRead = false,
    String reason = 'read',
  }) {
    return _getBool(
      MMKVKey.isUnexpectedExitNotificationEnabled,
      defaultValue: defaultValue,
      logRead: logRead,
      reason: reason,
    );
  }

  bool setUnexpectedExitNotificationEnabled(
    bool value, {
    required String reason,
  }) {
    return _putBool(
      MMKVKey.isUnexpectedExitNotificationEnabled,
      value,
      reason: reason,
    );
  }

  bool _getBool(
    String key, {
    required bool defaultValue,
    required bool logRead,
    required String reason,
  }) {
    final exists = MMKVUtil.containsKey(key);
    final value = MMKVUtil.getBool(key, defaultValue: defaultValue);
    if (logRead) {
      log.info(
        '[PermissionPrefs] read key=$key exists=$exists '
        'default=$defaultValue value=$value reason=$reason',
      );
    }
    return value;
  }

  bool _putBool(
    String key,
    bool value, {
    required String reason,
  }) {
    final existedBefore = MMKVUtil.containsKey(key);
    final previousValue =
        existedBefore ? MMKVUtil.getBool(key, defaultValue: false) : null;
    final success = MMKVUtil.putBool(key, value);
    final existsAfter = MMKVUtil.containsKey(key);
    final currentValue =
        existsAfter ? MMKVUtil.getBool(key, defaultValue: false) : null;
    if (!success || !existedBefore || previousValue != value) {
      log.info(
        '[PermissionPrefs] write key=$key existedBefore=$existedBefore '
        'previous=$previousValue new=$value success=$success '
        'existsAfter=$existsAfter current=$currentValue reason=$reason',
      );
    }
    return success;
  }
}
