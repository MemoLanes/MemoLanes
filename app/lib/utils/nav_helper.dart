import 'package:flutter/material.dart';
import 'package:memolanes/common/component/permission_request_sheet.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/service/permission_service.dart';

/// Root [Navigator] key shared across the app (dialogs, permission flow, share handler).
final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

/// Unified navigation helpers.
///
/// All helpers here wrap the destination page with [GlobalPopScope] so that
/// back button / back gesture is blocked while global loading is active.
Future<T?> navigatorPush<T>(
  BuildContext context, {
  required Widget page,
  RouteSettings? settings,
  bool fullscreenDialog = false,
  bool maintainState = true,
  bool rootNavigator = false,
}) {
  return Navigator.of(context, rootNavigator: rootNavigator).push<T>(
    MaterialPageRoute<T>(
      builder: (_) => GlobalPopScope(child: page),
      settings: settings,
      fullscreenDialog: fullscreenDialog,
      maintainState: maintainState,
    ),
  );
}

// --- Permission sheet (needs [navigatorKey], kept out of [PermissionService]) ---

/// First launch only: if any permission is still needed and the sheet was never shown,
/// show it once and persist in MMKV.
Future<void> tryShowPermissionSheetIfFirstTime() async {
  try {
    log.info('[PermissionFlow] first-launch permission sheet check started');
    final sheetShown = MMKVUtil.getBool(
      MMKVKey.permissionSheetShown,
      defaultValue: false,
    );
    if (sheetShown) {
      log.info(
        '[PermissionFlow] skip first-launch permission sheet: already shown',
      );
      return;
    }

    final needAny = await PermissionService().needAnyPermission();
    if (!needAny) {
      MMKVUtil.putBool(MMKVKey.permissionSheetShown, true);
      log.info(
        '[PermissionFlow] skip first-launch permission sheet: no permission needed',
      );
      return;
    }

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) {
      log.warning(
        '[PermissionFlow] cannot show first-launch permission sheet: context unavailable',
      );
      return;
    }

    log.info('[PermissionFlow] showing first-launch permission sheet');
    await showPermissionRequestSheet(context);
    MMKVUtil.putBool(MMKVKey.permissionSheetShown, true);
  } catch (e, s) {
    log.error("[NavHelper] tryShowPermissionSheetIfFirstTime $e", s);
  }
}

/// User-driven (e.g. record / map): location is required, while notification
/// and battery permissions are optional follow-ups handled by the sheet.
Future<bool> checkAndRequestPermission() async {
  try {
    final svc = PermissionService();
    if (await svc.checkLocationPermission()) {
      return true;
    }

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) {
      final hasLocation = await svc.checkLocationPermission();
      log.warning(
        '[PermissionFlow] cannot show user-driven permission sheet: '
        'context unavailable hasLocation=$hasLocation',
      );
      return hasLocation;
    }

    log.info('[PermissionFlow] showing user-driven permission sheet');
    await showPermissionRequestSheet(context);
    final hasLocation = await svc.checkLocationPermission();
    log.info(
      '[PermissionFlow] user-driven permission sheet closed '
      'hasLocation=$hasLocation',
    );
    return hasLocation;
  } catch (e, s) {
    log.error("[NavHelper] checkAndRequestPermission $e", s);
    return false;
  }
}
