import 'dart:io';

import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:permission_handler/permission_handler.dart';

/// Single side-effect step the UI layer should perform (dialogs / system pages).
/// No [BuildContext]: [showPermissionRequestSheet] applies these with [showCommonDialog] etc.
class PermissionEffect {
  final String? messageTrKey;
  final bool openAppSettings;
  final bool openLocationSettings;

  const PermissionEffect({
    this.messageTrKey,
    this.openAppSettings = false,
    this.openLocationSettings = false,
  });
}

/// Read-only view of OS + MMKV state for the permission sheet tiles.
class PermissionSnapshot {
  final bool locationTileGranted;
  final bool locationPermanentlyDenied;
  final bool batteryTileGranted;
  final bool notificationTileGranted;
  final bool notificationPermanentlyDenied;

  const PermissionSnapshot({
    required this.locationTileGranted,
    required this.locationPermanentlyDenied,
    required this.batteryTileGranted,
    required this.notificationTileGranted,
    required this.notificationPermanentlyDenied,
  });
}

/// Location / notification / battery checks and request flows only — no Flutter UI.
class PermissionService {
  PermissionService._privateConstructor();

  static final PermissionService _instance =
      PermissionService._privateConstructor();

  factory PermissionService() => _instance;

  Future<PermissionSnapshot> readPermissionSnapshot() async {
    final locStatus = await Permission.location.status;
    final locAlwaysStatus = await Permission.locationAlways.status;
    final isAndroid = Platform.isAndroid;
    final batteryGranted =
        !isAndroid || await Permission.ignoreBatteryOptimizations.isGranted;
    final notificationStatus = await Permission.notification.status;
    final notificationGranted = notificationStatus.isGranted;
    final hasLocation = locStatus.isGranted || locAlwaysStatus.isGranted;

    return PermissionSnapshot(
      locationTileGranted: hasLocation,
      locationPermanentlyDenied: locStatus.isPermanentlyDenied,
      batteryTileGranted: batteryGranted,
      notificationTileGranted: notificationGranted,
      notificationPermanentlyDenied: notificationStatus.isPermanentlyDenied,
    );
  }

  Future<bool> needAnyPermission() async {
    final hasLocation = await checkLocationPermission();
    if (!hasLocation) return true;
    if (Platform.isAndroid &&
        !(await Permission.ignoreBatteryOptimizations.isGranted) &&
        !MMKVUtil.getBool(MMKVKey.requestedBatteryOptimization,
            defaultValue: false)) {
      return true;
    }
    final notificationStatus = await Permission.notification.status;
    if (!notificationStatus.isGranted &&
        !MMKVUtil.getBool(MMKVKey.requestedNotification, defaultValue: false) &&
        !notificationStatus.isPermanentlyDenied) {
      return true;
    }
    return false;
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
    } catch (_) {
      return false;
    }
  }

  /// GPS off → open system location page. No pre-request dialogs.
  Future<List<PermissionEffect>> runLocationRequest() async {
    if (!await Geolocator.isLocationServiceEnabled()) {
      return const [
        PermissionEffect(openLocationSettings: true),
      ];
    }

    var status = await Permission.location.status;

    if (status.isPermanentlyDenied) {
      return [
        PermissionEffect(
          messageTrKey:
              'location_service.location_permission_permanently_denied',
          openAppSettings: true,
        ),
      ];
    }

    if (!status.isGranted) {
      status = await Permission.location.request();
      if (!status.isGranted) {
        final deniedMessageKey = status.isPermanentlyDenied
            ? 'location_service.location_permission_permanently_denied'
            : 'location_service.location_permission_denied';
        return [
          PermissionEffect(
            messageTrKey: deniedMessageKey,
            openAppSettings: status.isPermanentlyDenied,
          ),
        ];
      }
    }

    if (status.isGranted && Platform.isIOS) {
      await Permission.locationAlways.request();
    }

    return const [];
  }

  Future<List<PermissionEffect>> runBatteryRequest() async {
    if (!Platform.isAndroid) return const [];

    final alreadyRequested = MMKVUtil.getBool(
      MMKVKey.requestedBatteryOptimization,
      defaultValue: false,
    );
    if (alreadyRequested) {
      final ignoring = await Permission.ignoreBatteryOptimizations.isGranted;
      if (!ignoring) {
        return [
          const PermissionEffect(
            messageTrKey: 'location_service.battery_optimization_denied',
          ),
        ];
      }
      return const [];
    }

    final result = await Permission.ignoreBatteryOptimizations.request();
    MMKVUtil.putBool(MMKVKey.requestedBatteryOptimization, true);
    if (!result.isGranted) {
      return [
        const PermissionEffect(
          messageTrKey: 'location_service.battery_optimization_denied',
        ),
      ];
    }
    return const [];
  }

  Future<List<PermissionEffect>> runNotificationRequest() async {
    final status = await Permission.notification.status;
    final alreadyRequested = MMKVUtil.getBool(
      MMKVKey.requestedNotification,
      defaultValue: false,
    );

    if (status.isGranted) {
      MMKVUtil.putBool(MMKVKey.isUnexpectedExitNotificationEnabled, true);
      return const [];
    }

    if (status.isPermanentlyDenied) {
      return [
        const PermissionEffect(
          messageTrKey:
              'unexpected_exit_notification.notification_permission_denied',
          openAppSettings: true,
        ),
      ];
    }

    if (alreadyRequested) {
      return [
        const PermissionEffect(
          messageTrKey:
              'unexpected_exit_notification.notification_permission_denied',
        ),
      ];
    }

    final result = await Permission.notification.request();
    MMKVUtil.putBool(
      MMKVKey.isUnexpectedExitNotificationEnabled,
      result.isGranted,
    );
    MMKVUtil.putBool(MMKVKey.requestedNotification, true);
    if (!result.isGranted) {
      return [
        const PermissionEffect(
          messageTrKey:
              'unexpected_exit_notification.notification_permission_denied',
        ),
      ];
    }
    return const [];
  }
}
