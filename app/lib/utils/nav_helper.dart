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
    final sheetShown =
        MMKVUtil.getBool(MMKVKey.permissionSheetShown, defaultValue: false);
    if (sheetShown) return;

    final needAny = await PermissionService().needAnyPermission();
    if (!needAny) return;

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;

    final entered = await showPermissionRequestSheet(context);
    if (entered) {
      MMKVUtil.putBool(MMKVKey.permissionSheetShown, true);
    }
  } catch (e) {
    log.error("[NavHelper] tryShowPermissionSheetIfFirstTime $e");
  }
}

/// User-driven (e.g. record / map): if any permission is still needed, show the sheet.
Future<bool> checkAndRequestPermission() async {
  try {
    final svc = PermissionService();
    final needAny = await svc.needAnyPermission();
    if (!needAny) return await svc.checkLocationPermission();

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) {
      return await svc.checkLocationPermission();
    }

    await showPermissionRequestSheet(context);
    return await svc.checkLocationPermission();
  } catch (e) {
    log.error("[NavHelper] checkAndRequestPermission $e");
    return false;
  }
}
