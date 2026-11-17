import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/utils/nav_helper.dart' show navigatorKey;
import 'package:permission_handler/permission_handler.dart';

const _requestedBackgroundLocation = 'Permission.requestedBackgroundLocation';

/// Requires background location before starting a Tracelet recording.
Future<bool> offerTraceletRecordingPermission() async {
  try {
    if (await Permission.locationAlways.isGranted) return true;
    final wasOffered = MMKVUtil.getBool(_requestedBackgroundLocation);
    final openSettings =
        wasOffered || defaultTargetPlatform == TargetPlatform.android;
    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return false;
    final confirmed = await showCommonDialog(
      context,
      context.tr('location_service.always_location_required'),
      hasCancel: true,
      confirmButtonText: openSettings
          ? context.tr('permission_sheet.open_system_settings')
          : null,
    );
    if (!context.mounted) return false;
    if (confirmed) {
      if (openSettings) {
        if (!await openAppSettings()) {
          throw StateError(
            'Could not open app settings for background location',
          );
        }
      } else {
        final result = await Permission.locationAlways.request();
        log.info('[Tracelet] Recording background location result=$result');
      }
    }
    if (!wasOffered && !MMKVUtil.putBool(_requestedBackgroundLocation, true)) {
      throw StateError('Could not save the background location prompt state');
    }
    if (confirmed && openSettings) return false;
    return await Permission.locationAlways.isGranted;
  } catch (error, stackTrace) {
    log.error(
      '[Tracelet] Requesting background location failed: $error',
      stackTrace,
    );
    return false;
  }
}
