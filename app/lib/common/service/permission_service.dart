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

class PermissionTileStatus {
  final bool granted;
  final bool denied;
  final bool permanentlyDenied;

  const PermissionTileStatus({
    required this.granted,
    this.denied = false,
    this.permanentlyDenied = false,
  });
}

/// Read-only view of OS + MMKV state for the permission sheet tiles.
class PermissionSnapshot {
  final PermissionTileStatus location;
  final PermissionTileStatus battery;
  final PermissionTileStatus notification;

  const PermissionSnapshot({
    required this.location,
    required this.battery,
    required this.notification,
  });
}

/// Location / notification / battery checks and request flows only — no Flutter UI.
class PermissionService {
  PermissionService._privateConstructor();

  static final PermissionService _instance =
      PermissionService._privateConstructor();

  factory PermissionService() => _instance;

  Future<PermissionSnapshot> readPermissionSnapshot() async {
    final hasLocation = await checkLocationPermission();
    final locStatus = await Permission.location.status;
    final isAndroid = Platform.isAndroid;
    final batteryGranted =
        !isAndroid || await Permission.ignoreBatteryOptimizations.isGranted;
    final notificationStatus = await Permission.notification.status;
    final notificationGranted = notificationStatus.isGranted;
    final locationRequested =
        MMKVUtil.getBool(MMKVKey.requestedLocation, defaultValue: false);
    final batteryRequested = MMKVUtil.getBool(
      MMKVKey.requestedBatteryOptimization,
      defaultValue: false,
    );
    final notificationRequested = MMKVUtil.getBool(
      MMKVKey.requestedNotification,
      defaultValue: false,
    );

    return PermissionSnapshot(
      location: PermissionTileStatus(
        granted: hasLocation,
        denied:
            !hasLocation && locationRequested && !locStatus.isPermanentlyDenied,
        permanentlyDenied: locStatus.isPermanentlyDenied,
      ),
      battery: PermissionTileStatus(
        granted: batteryGranted,
        denied: isAndroid && !batteryGranted && batteryRequested,
      ),
      notification: PermissionTileStatus(
        granted: notificationGranted,
        denied: !notificationGranted &&
            notificationRequested &&
            !notificationStatus.isPermanentlyDenied,
        permanentlyDenied: notificationStatus.isPermanentlyDenied,
      ),
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
      MMKVUtil.putBool(MMKVKey.requestedLocation, true);
      return const [
        PermissionEffect(
          messageTrKey:
              'location_service.location_permission_permanently_denied',
          openAppSettings: true,
        ),
      ];
    }

    if (!status.isGranted) {
      MMKVUtil.putBool(MMKVKey.requestedLocation, true);
      status = await Permission.location.request();
      if (!status.isGranted) {
        if (status.isPermanentlyDenied) {
          return const [
            PermissionEffect(
              messageTrKey:
                  'location_service.location_permission_permanently_denied',
              openAppSettings: true,
            ),
          ];
        }
        return const [
          PermissionEffect(
            messageTrKey: 'location_service.location_permission_denied',
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
    if (!Platform.isAndroid) {
      return const [];
    }

    final alreadyRequested = MMKVUtil.getBool(
      MMKVKey.requestedBatteryOptimization,
      defaultValue: false,
    );
    if (alreadyRequested) {
      final ignoring = await Permission.ignoreBatteryOptimizations.isGranted;
      if (ignoring) return const [];
    }

    // ignoreBatteryOptimizations is a "special permission" on Android — request()
    // launches system settings and returns the current status immediately without
    // waiting for the user to return. The actual result will be picked up when the
    // app resumes (didChangeAppLifecycleState → _refreshStatus).
    await Permission.ignoreBatteryOptimizations.request();
    MMKVUtil.putBool(MMKVKey.requestedBatteryOptimization, true);
    return const [];
  }

  Future<List<PermissionEffect>> runNotificationRequest() async {
    final status = await Permission.notification.status;

    if (status.isGranted) {
      MMKVUtil.putBool(MMKVKey.isUnexpectedExitNotificationEnabled, true);
      return const [];
    }

    if (status.isPermanentlyDenied) {
      MMKVUtil.putBool(MMKVKey.requestedNotification, true);
      return const [
        PermissionEffect(
          messageTrKey:
              'unexpected_exit_notification.notification_permission_denied',
          openAppSettings: true,
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
      if (result.isPermanentlyDenied) {
        return const [
          PermissionEffect(
            messageTrKey:
                'unexpected_exit_notification.notification_permission_denied',
            openAppSettings: true,
          ),
        ];
      }
      return const [
        PermissionEffect(
          messageTrKey:
              'unexpected_exit_notification.notification_permission_denied',
        ),
      ];
    }
    return const [];
  }
}
