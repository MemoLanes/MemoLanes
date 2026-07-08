import 'dart:io';

import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/permission_prefs.dart';
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

  final PermissionPrefs _prefs = PermissionPrefs();

  Future<PermissionSnapshot> readPermissionSnapshot() async {
    final locStatus = await Permission.location.status;
    final locAlwaysStatus = await Permission.locationAlways.status;
    final isAndroid = Platform.isAndroid;
    final batteryGranted =
        !isAndroid || await Permission.ignoreBatteryOptimizations.isGranted;
    final notificationStatus = await Permission.notification.status;
    final notificationGranted = notificationStatus.isGranted;
    final hasLocation = locStatus.isGranted || locAlwaysStatus.isGranted;
    final locationRequested = _prefs.getRequestedLocation(
      reason: 'read_permission_snapshot',
    );
    final batteryRequested = _prefs.getRequestedBatteryOptimization(
      reason: 'read_permission_snapshot',
    );
    final notificationRequested = _prefs.getRequestedNotification(
      reason: 'read_permission_snapshot',
    );

    final snapshot = PermissionSnapshot(
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
    return snapshot;
  }

  Future<bool> needAnyPermission() async {
    final hasLocation = await checkLocationPermission();
    if (!hasLocation) {
      log.info(
        '[PermissionService] needAnyPermission=true reason=location_missing',
      );
      return true;
    }
    if (Platform.isAndroid &&
        !(await Permission.ignoreBatteryOptimizations.isGranted) &&
        !_prefs.getRequestedBatteryOptimization(
          reason: 'need_any_permission',
        )) {
      log.info(
        '[PermissionService] needAnyPermission=true '
        'reason=battery_optimization_not_requested',
      );
      return true;
    }
    final notificationStatus = await Permission.notification.status;
    if (!notificationStatus.isGranted &&
        !_prefs.getRequestedNotification(reason: 'need_any_permission') &&
        !notificationStatus.isPermanentlyDenied) {
      log.info(
        '[PermissionService] needAnyPermission=true '
        'reason=notification_not_requested status=$notificationStatus',
      );
      return true;
    }
    log.info(
      '[PermissionService] needAnyPermission=false '
      'notificationStatus=$notificationStatus',
    );
    return false;
  }

  Future<bool> checkLocationPermission() async {
    try {
      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        return false;
      }
      final granted = await Permission.location.isGranted;
      if (!granted) {
        return false;
      }
      return true;
    } catch (e, s) {
      log.error('[PermissionService] checkLocationPermission failed $e', s);
      return false;
    }
  }

  /// GPS off → open system location page. No pre-request dialogs.
  Future<List<PermissionEffect>> runLocationRequest() async {
    if (!await Geolocator.isLocationServiceEnabled()) {
      log.info(
        '[PermissionService] runLocationRequest open location settings: '
        'location service disabled',
      );
      return const [
        PermissionEffect(openLocationSettings: true),
      ];
    }

    var status = await Permission.location.status;

    if (status.isPermanentlyDenied) {
      _prefs.setRequestedLocation(
        true,
        reason: 'location_permanently_denied_before_request',
      );
      log.info(
        '[PermissionService] runLocationRequest open app settings: '
        'status=$status',
      );
      return const [
        PermissionEffect(
          messageTrKey:
              'location_service.location_permission_permanently_denied',
          openAppSettings: true,
        ),
      ];
    }

    if (!status.isGranted) {
      _prefs.setRequestedLocation(
        true,
        reason: 'before_location_request',
      );
      status = await Permission.location.request();
      log.info(
        '[PermissionService] runLocationRequest requested result=$status',
      );
      if (!status.isGranted) {
        if (status.isPermanentlyDenied) {
          log.info(
            '[PermissionService] runLocationRequest result=permanentlyDenied',
          );
          return const [
            PermissionEffect(
              messageTrKey:
                  'location_service.location_permission_permanently_denied',
              openAppSettings: true,
            ),
          ];
        }
        log.info('[PermissionService] runLocationRequest result=denied');
        return const [
          PermissionEffect(
            messageTrKey: 'location_service.location_permission_denied',
          ),
        ];
      }
    }

    if (status.isGranted && Platform.isIOS) {
      final alwaysResult = await Permission.locationAlways.request();
      log.info(
        '[PermissionService] runLocationRequest iOS always result=$alwaysResult',
      );
    }

    return const [];
  }

  Future<List<PermissionEffect>> runBatteryRequest() async {
    if (!Platform.isAndroid) {
      return const [];
    }

    final alreadyRequested = _prefs.getRequestedBatteryOptimization(
      reason: 'run_battery_request',
    );
    if (alreadyRequested) {
      final ignoring = await Permission.ignoreBatteryOptimizations.isGranted;
      if (ignoring) {
        return const [];
      }
    }

    // ignoreBatteryOptimizations is a "special permission" on Android — request()
    // launches system settings and returns the current status immediately without
    // waiting for the user to return. The actual result will be picked up when the
    // app resumes (didChangeAppLifecycleState → _refreshStatus).
    await Permission.ignoreBatteryOptimizations.request();
    _prefs.setRequestedBatteryOptimization(
      true,
      reason: 'after_battery_optimization_request',
    );
    log.info('[PermissionService] runBatteryRequest launched system request');
    return const [];
  }

  Future<List<PermissionEffect>> runNotificationRequest() async {
    final status = await Permission.notification.status;

    if (status.isGranted) {
      _prefs.setUnexpectedExitNotificationEnabled(
        true,
        reason: 'notification_already_granted',
      );
      return const [];
    }

    if (status.isPermanentlyDenied) {
      _prefs.setRequestedNotification(
        true,
        reason: 'notification_permanently_denied_before_request',
      );
      log.info(
        '[PermissionService] runNotificationRequest open app settings: '
        'status=$status',
      );
      return const [
        PermissionEffect(
          messageTrKey:
              'unexpected_exit_notification.notification_permission_denied',
          openAppSettings: true,
        ),
      ];
    }

    final result = await Permission.notification.request();
    log.info(
      '[PermissionService] runNotificationRequest requested result=$result',
    );
    _prefs.setUnexpectedExitNotificationEnabled(
      result.isGranted,
      reason: 'after_notification_request',
    );
    _prefs.setRequestedNotification(
      true,
      reason: 'after_notification_request',
    );
    if (!result.isGranted) {
      if (result.isPermanentlyDenied) {
        log.info(
          '[PermissionService] runNotificationRequest result=permanentlyDenied',
        );
        return const [
          PermissionEffect(
            messageTrKey:
                'unexpected_exit_notification.notification_permission_denied',
            openAppSettings: true,
          ),
        ];
      }
      log.info(
        '[PermissionService] runNotificationRequest result=denied',
      );
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
