import 'dart:io';

import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/component/permission_request_sheet.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/main.dart';
import 'package:permission_handler/permission_handler.dart';

class PermissionService {
  PermissionService._privateConstructor();

  static final PermissionService _instance =
      PermissionService._privateConstructor();

  factory PermissionService() => _instance;

  /// 仅在首次打开 app 时调用：若需要权限且未弹过层，则弹出一次并写入 MMKV。
  /// 之后打开 app 不再主动弹出。
  Future<void> tryShowPermissionSheetIfFirstTime() async {
    try {
      final sheetShown =
          MMKVUtil.getBool(MMKVKey.permissionSheetShown, defaultValue: false);
      if (sheetShown) return;

      final needAny = await _needAnyPermission();
      if (!needAny) return;

      final context = navigatorKey.currentState?.context;
      if (context == null || !context.mounted) return;

      await showPermissionRequestSheet(context);
      MMKVUtil.putBool(MMKVKey.permissionSheetShown, true);
    } catch (e) {
      log.error("[PermissionService] tryShowPermissionSheetIfFirstTime $e");
    }
  }

  Future<bool> _needAnyPermission() async {
    final hasLocation = await checkLocationPermission();
    if (!hasLocation) return true;
    if (Platform.isAndroid &&
        !(await Permission.ignoreBatteryOptimizations.isGranted) &&
        !MMKVUtil.getBool(MMKVKey.requestedBatteryOptimization,
            defaultValue: false)) {
      return true;
    }
    if (!(await Permission.notification.status.isGranted) &&
        !MMKVUtil.getBool(MMKVKey.requestedNotification, defaultValue: false) &&
        !(await Permission.notification.status.isPermanentlyDenied)) {
      return true;
    }
    return false;
  }

  /// 用户触发（点录制/定位）时调用：需要权限则始终弹出权限层。
  Future<bool> checkAndRequestPermission() async {
    try {
      final needAny = await _needAnyPermission();
      if (!needAny) return await checkLocationPermission();

      final context = navigatorKey.currentState?.context;
      if (context == null || !context.mounted) {
        return await checkLocationPermission();
      }

      await showPermissionRequestSheet(context);
      return await checkLocationPermission();
    } catch (e) {
      log.error("[PermissionService] checkAndRequestPermission failed $e");
      return false;
    }
  }

  Future<bool> checkLocationPermission() async {
    try {
      if (!await Geolocator.isLocationServiceEnabled()) {
        return false;
      }
      if (!await Permission.location.isGranted) {
        return false;
      }
      return true;
    } catch (e) {
      return false;
    }
  }
}
