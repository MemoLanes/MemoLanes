import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
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
  final PermissionTileStatus motion;

  const PermissionSnapshot({
    required this.location,
    required this.battery,
    required this.notification,
    required this.motion,
  });
}

/// Permission checks and request flows only — no Flutter UI.
class PermissionService {
  PermissionService._privateConstructor();

  static final PermissionService _instance =
      PermissionService._privateConstructor();

  factory PermissionService() => _instance;

  Permission get motionPermission => switch (defaultTargetPlatform) {
    TargetPlatform.android => Permission.activityRecognition,
    _ => Permission.sensors,
  };

  Permission get _locationPermission =>
      defaultTargetPlatform == TargetPlatform.iOS
      ? Permission.locationWhenInUse
      : Permission.location;

  bool _isPermanentlyDeniedFromStatus(PermissionStatus status) {
    // Android status cannot distinguish permanent denial from "Ask every time".
    // Only request() results can report permanent denial there.
    return !Platform.isAndroid && status.isPermanentlyDenied;
  }

  Future<void> logPermissionState(String event) async {
    try {
      final locationServiceEnabled =
          await Geolocator.isLocationServiceEnabled();
      final locationStatus = await _locationPermission.status;
      final batteryOptimizationStatus = Platform.isAndroid
          ? '${await Permission.ignoreBatteryOptimizations.status}'
          : 'n/a';
      final notificationStatus = await Permission.notification.status;
      final motionStatus = await motionPermission.status;
      final permissionSheetShown = MMKVUtil.getBool(
        MMKVKey.permissionSheetShown,
        defaultValue: false,
      );
      final requestedLocation = MMKVUtil.getBool(
        MMKVKey.requestedLocation,
        defaultValue: false,
      );
      final requestedBatteryOptimization = MMKVUtil.getBool(
        MMKVKey.requestedBatteryOptimization,
        defaultValue: false,
      );
      final requestedNotification = MMKVUtil.getBool(
        MMKVKey.requestedNotification,
        defaultValue: false,
      );
      final requestedMotion = MMKVUtil.getBool(MMKVKey.requestedMotion);
      final unexpectedExitNotificationEnabled = MMKVUtil.getBool(
        MMKVKey.isUnexpectedExitNotificationEnabled,
        defaultValue: true,
      );

      log.info(
        '[PermissionState] event=$event '
        'os={locationServiceEnabled=$locationServiceEnabled '
        'location=$locationStatus '
        'batteryOptimization=$batteryOptimizationStatus '
        'notification=$notificationStatus motion=$motionStatus} '
        'mmkv={permissionSheetShown=$permissionSheetShown '
        'requestedLocation=$requestedLocation '
        'requestedBatteryOptimization=$requestedBatteryOptimization '
        'requestedNotification=$requestedNotification '
        'requestedMotion=$requestedMotion '
        'unexpectedExitNotificationEnabled='
        '$unexpectedExitNotificationEnabled}',
      );
    } catch (e, s) {
      log.error('[PermissionState] log failed event=$event $e', s);
    }
  }

  Future<PermissionSnapshot> readPermissionSnapshot() async {
    final hasLocation = await checkLocationPermission();
    final locStatus = await _locationPermission.status;
    final isAndroid = Platform.isAndroid;
    final batteryGranted =
        !isAndroid || await Permission.ignoreBatteryOptimizations.isGranted;
    final notificationStatus = await Permission.notification.status;
    final motionStatus = await motionPermission.status;
    final notificationGranted = notificationStatus.isGranted;
    final locationPermanentlyDenied = _isPermanentlyDeniedFromStatus(locStatus);
    final notificationPermanentlyDenied = _isPermanentlyDeniedFromStatus(
      notificationStatus,
    );
    final locationRequested = MMKVUtil.getBool(
      MMKVKey.requestedLocation,
      defaultValue: false,
    );
    final batteryRequested = MMKVUtil.getBool(
      MMKVKey.requestedBatteryOptimization,
      defaultValue: false,
    );
    final notificationRequested = MMKVUtil.getBool(
      MMKVKey.requestedNotification,
      defaultValue: false,
    );
    final motionRequested = MMKVUtil.getBool(MMKVKey.requestedMotion);

    final snapshot = PermissionSnapshot(
      location: PermissionTileStatus(
        granted: hasLocation,
        denied: !hasLocation && locationRequested && !locationPermanentlyDenied,
        permanentlyDenied: locationPermanentlyDenied,
      ),
      battery: PermissionTileStatus(
        granted: batteryGranted,
        denied: isAndroid && !batteryGranted && batteryRequested,
      ),
      notification: PermissionTileStatus(
        granted: notificationGranted,
        denied:
            !notificationGranted &&
            notificationRequested &&
            !notificationPermanentlyDenied,
        permanentlyDenied: notificationPermanentlyDenied,
      ),
      motion: PermissionTileStatus(
        granted: motionStatus.isGranted,
        denied:
            !motionStatus.isGranted &&
            motionRequested &&
            !motionStatus.isPermanentlyDenied,
        permanentlyDenied: motionStatus.isPermanentlyDenied,
      ),
    );
    return snapshot;
  }

  Future<bool> needAnyPermission() async {
    final hasLocation = await checkLocationPermission();
    if (!hasLocation) {
      return true;
    }
    if (Platform.isAndroid &&
        !(await Permission.ignoreBatteryOptimizations.isGranted) &&
        !MMKVUtil.getBool(
          MMKVKey.requestedBatteryOptimization,
          defaultValue: false,
        )) {
      return true;
    }
    final notificationStatus = await Permission.notification.status;
    if (!notificationStatus.isGranted &&
        !MMKVUtil.getBool(MMKVKey.requestedNotification, defaultValue: false) &&
        !_isPermanentlyDeniedFromStatus(notificationStatus)) {
      return true;
    }
    final motionStatus = await motionPermission.status;
    if (!motionStatus.isGranted &&
        !MMKVUtil.getBool(MMKVKey.requestedMotion) &&
        !motionStatus.isPermanentlyDenied) {
      return true;
    }
    return false;
  }

  Future<bool> checkLocationPermission() async {
    try {
      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        return false;
      }
      final granted = await _locationPermission.isGranted;
      if (!granted) {
        return false;
      }
      return true;
    } catch (e, s) {
      log.error('[PermissionService] checkLocationPermission failed $e', s);
      return false;
    }
  }

  Future<bool> checkBackgroundLocationPermission() =>
      Permission.locationAlways.isGranted;

  /// GPS off → open system location page. No pre-request dialogs.
  Future<List<PermissionEffect>> runLocationRequest() async {
    if (!await Geolocator.isLocationServiceEnabled()) {
      log.info(
        '[PermissionService] runLocationRequest open location settings: '
        'location service disabled',
      );
      return const [PermissionEffect(openLocationSettings: true)];
    }

    final permission = _locationPermission;
    var status = await permission.status;

    if (_isPermanentlyDeniedFromStatus(status)) {
      MMKVUtil.putBool(MMKVKey.requestedLocation, true);
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
      MMKVUtil.putBool(MMKVKey.requestedLocation, true);
      status = await permission.request();
      log.info(
        '[PermissionService] runLocationRequest requested result=$status',
      );
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
      if (ignoring) {
        return const [];
      }
    }

    // ignoreBatteryOptimizations is a "special permission" on Android — request()
    // launches system settings and returns the current status immediately without
    // waiting for the user to return. The actual result will be picked up when the
    // app resumes (didChangeAppLifecycleState → _refreshStatus).
    await Permission.ignoreBatteryOptimizations.request();
    MMKVUtil.putBool(MMKVKey.requestedBatteryOptimization, true);
    log.info('[PermissionService] runBatteryRequest launched system request');
    return const [];
  }

  Future<List<PermissionEffect>> runNotificationRequest() async {
    final status = await Permission.notification.status;

    if (status.isGranted) {
      MMKVUtil.putBool(MMKVKey.isUnexpectedExitNotificationEnabled, true);
      return const [];
    }

    if (_isPermanentlyDeniedFromStatus(status)) {
      MMKVUtil.putBool(MMKVKey.requestedNotification, true);
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

  Future<List<PermissionEffect>> runMotionRequest() async {
    final status = await motionPermission.status;
    if (status.isGranted) return const [];
    if (!MMKVUtil.putBool(MMKVKey.requestedMotion, true)) {
      throw StateError('Could not save the motion permission request state');
    }
    if (status.isPermanentlyDenied) {
      log.info(
        '[PermissionService] runMotionRequest open app settings: status=$status',
      );
      return const [
        PermissionEffect(
          messageTrKey: 'permission_sheet.motion_permission_denied',
          openAppSettings: true,
        ),
      ];
    }
    final result = await motionPermission.request();
    log.info('[PermissionService] runMotionRequest requested result=$result');
    if (!result.isGranted) {
      return [
        PermissionEffect(
          messageTrKey: 'permission_sheet.motion_permission_denied',
          openAppSettings: result.isPermanentlyDenied,
        ),
      ];
    }
    return const [];
  }
}
