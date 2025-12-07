import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/main.dart';
import 'package:permission_handler/permission_handler.dart';

class PermissionService {
  PermissionService._privateConstructor();

  static final PermissionService _instance =
      PermissionService._privateConstructor();

  factory PermissionService() => _instance;

  Future<bool> checkAndRequestPermission() async {
    try {
      bool hasPermission = await checkLocationPermission();
      if (!hasPermission) {
        await _requestLocationPermission();
        hasPermission = await checkLocationPermission();
      }
      await _requestIgnoreBatteryOptimization();
      await _requestNotificationPermission();
      return hasPermission;
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
      if (!(await Permission.location.isGranted ||
          await Permission.locationAlways.isGranted)) {
        return false;
      }
      return true;
    } catch (e) {
      return false;
    }
  }

  Future<void> _showPermissionDeniedDialog(String message) async {
    final context = navigatorKey.currentState?.context;
    if (context != null && context.mounted) {
      await showCommonDialog(context, message);
    }
  }

  Future<void> _requestLocationPermission() async {
    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;

    if (!await Geolocator.isLocationServiceEnabled()) {
      await _showPermissionDeniedDialog(
        context.tr("location_service.location_service_disabled"),
      );
      if (!await Geolocator.openLocationSettings()) {
        throw Exception("Location services not enabled.");
      }
    }

    var locStatus = await Permission.location.status;
    if (locStatus.isPermanentlyDenied) {
      await _showPermissionDeniedDialog(
        context.tr("location_service.location_permission_permanently_denied"),
      );
      await openAppSettings();
      throw Exception("Location permission permanently denied.");
    }

    if (!locStatus.isGranted) {
      await _showPermissionDeniedDialog(
        context.tr("location_service.location_permission_reason"),
      );
      locStatus = await Permission.location.request();
      if (!locStatus.isGranted) {
        await _showPermissionDeniedDialog(
          context.tr("location_service.location_permission_permanently_denied"),
        );
        throw Exception("Location permission not granted.");
      }
    }

    var bgStatus = await Permission.locationAlways.status;
    if (!bgStatus.isGranted) {
      bgStatus = await Permission.locationAlways.request();
      // It seems this does not wait for the result on iOS, and always
      // permission is not strictly required.
      if (Platform.isAndroid && !bgStatus.isGranted) {
        await _showPermissionDeniedDialog(
          context.tr(
              "location_service.background_location_permission_permanently_denied"),
        );
      }
    }
  }

  Future<void> _requestIgnoreBatteryOptimization() async {
    if (!Platform.isAndroid) return;

    final alreadyRequested = MMKVUtil.getBool(
        MMKVKey.requestBatteryOptimization,
        defaultValue: false);
    if (alreadyRequested) return;

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;

    final isIgnoring = await Permission.ignoreBatteryOptimizations.isGranted;
    if (!isIgnoring) {
      await _showPermissionDeniedDialog(
        context.tr("location_service.battery_optimization_reason"),
      );
      final result = await Permission.ignoreBatteryOptimizations.request();
      if (!result.isGranted) {
        await _showPermissionDeniedDialog(
          context.tr("location_service.battery_optimization_denied"),
        );
      }
    }
    MMKVUtil.putBool(MMKVKey.requestBatteryOptimization, true);
  }

  Future<void> _requestNotificationPermission() async {
    final status = await Permission.notification.status;

    final alreadyRequested = MMKVUtil.getBool(
      MMKVKey.requestNotification,
      defaultValue: false,
    );

    if (status.isGranted || alreadyRequested || status.isPermanentlyDenied) {
      if (status.isGranted) {
        MMKVUtil.putBool(MMKVKey.isUnexpectedExitNotificationEnabled, true);
      }
      return;
    }

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;
    await _showPermissionDeniedDialog(
      context.tr("unexpected_exit_notification.notification_permission_reason"),
    );

    final result = await Permission.notification.request();

    MMKVUtil.putBool(
      MMKVKey.isUnexpectedExitNotificationEnabled,
      result.isGranted,
    );

    if (!result.isGranted) {
      await _showPermissionDeniedDialog(
        context
            .tr("unexpected_exit_notification.notification_permission_denied"),
      );
    }
    MMKVUtil.putBool(MMKVKey.requestNotification, true);
  }
}
