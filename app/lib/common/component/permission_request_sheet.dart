import 'dart:async';
import 'dart:io' show Platform;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:permission_handler/permission_handler.dart';

/// Shows the unified permission request bottom sheet (layout + copy only).
///
/// Completes after the sheet is closed.
Future<void> showPermissionRequestSheet(
  BuildContext context,
) async {
  final permissions = PermissionService();
  unawaited(permissions.logPermissionState('sheet_open'));
  await showModalBottomSheet<void>(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    isDismissible: true,
    builder: (context) {
      return _PermissionRequestSheetContent();
    },
  );
  await permissions.logPermissionState('sheet_close');
}

class _PermissionRequestSheetContent extends StatefulWidget {
  @override
  State<_PermissionRequestSheetContent> createState() =>
      _PermissionRequestSheetContentState();
}

class _PermissionRequestSheetContentState
    extends State<_PermissionRequestSheetContent> with WidgetsBindingObserver {
  final PermissionService _permissions = PermissionService();

  PermissionTileStatus _location = const PermissionTileStatus(granted: false);
  PermissionTileStatus _battery = const PermissionTileStatus(granted: false);
  PermissionTileStatus _notification =
      const PermissionTileStatus(granted: false);
  bool _isClosing = false;

  Future<void> _showLocationRationaleDialog() async {
    if (!mounted) return;
    await showCommonDialog(
      context,
      context.tr('location_service.location_permission_reason'),
    );
  }

  Future<void> _showBatteryRationaleDialog() async {
    if (!mounted) return;
    await showCommonDialog(
      context,
      context.tr('location_service.battery_optimization_reason'),
    );
  }

  Future<void> _showNotificationRationaleDialog() async {
    if (!mounted) return;
    await showCommonDialog(
      context,
      context.tr('unexpected_exit_notification.notification_permission_reason'),
    );
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _refreshStatus(closeIfComplete: true);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      // Some special permissions (e.g. ignoreBatteryOptimizations) report stale
      // status immediately after resume. Delay to let the system sync.
      Future.delayed(const Duration(milliseconds: 200), () {
        _refreshStatus(closeIfComplete: true);
      });
    }
  }

  Future<void> _refreshStatus({bool closeIfComplete = false}) async {
    if (_isClosing) return;
    final s = await _permissions.readPermissionSnapshot();
    if (!mounted || _isClosing) return;
    setState(() {
      _location = s.location;
      _battery = s.battery;
      _notification = s.notification;
    });
    if (closeIfComplete && _hasNoRemainingPermissions(s)) {
      _closeSheet();
    }
  }

  bool _hasNoRemainingPermissions(PermissionSnapshot s) {
    return s.location.granted &&
        (!Platform.isAndroid || s.battery.granted) &&
        s.notification.granted;
  }

  void _closeSheet() {
    if (!mounted || _isClosing) return;

    _isClosing = true;
    if (!popCurrentRoute(context)) {
      log.warning(
          '[PermissionSheet] close failed: popCurrentRoute returned false');
      _isClosing = false;
    }
  }

  Future<void> _applyEffects(List<PermissionEffect> effects) async {
    for (final e in effects) {
      if (!mounted) return;
      if (e.messageTrKey != null) {
        await showCommonDialog(context, context.tr(e.messageTrKey!));
      }
      if (!mounted) return;
      if (e.openAppSettings) {
        await openAppSettings();
      }
      if (!mounted) return;
      if (e.openLocationSettings) {
        await Geolocator.openLocationSettings();
      }
    }
  }

  Future<void> _requestLocation({bool closeIfComplete = true}) async {
    if (!mounted) return;
    final effects = await _permissions.runLocationRequest();
    await _applyEffects(effects);
    await _refreshStatus(closeIfComplete: closeIfComplete);
  }

  Future<void> _requestBattery({bool closeIfComplete = true}) async {
    if (!Platform.isAndroid) return;
    if (_battery.granted) return;
    if (!mounted) return;
    final effects = await _permissions.runBatteryRequest();
    await _applyEffects(effects);
    await _refreshStatus(closeIfComplete: closeIfComplete);
  }

  Future<void> _requestNotification({bool closeIfComplete = true}) async {
    if (_notification.granted) return;
    if (!mounted) return;
    final effects = await _permissions.runNotificationRequest();
    await _applyEffects(effects);
    await _refreshStatus(closeIfComplete: closeIfComplete);
  }

  void _onSkip() {
    _closeSheet();
  }

  Future<void> _onEnableAll() async {
    await _refreshStatus();
    if (!mounted) return;
    if (!_location.granted) await _requestLocation(closeIfComplete: false);
    if (!mounted) return;
    if (Platform.isAndroid && !_battery.granted) {
      await _requestBattery(closeIfComplete: false);
    }
    if (!mounted) return;
    if (!_notification.granted) {
      await _requestNotification(closeIfComplete: false);
    }
    if (!mounted) return;
    _closeSheet();
  }

  @override
  Widget build(BuildContext context) {
    return SetupBottomSheet(
      title: context.tr("permission_sheet.title"),
      maxHeightFactor: 0.6,
      leading: IconButton(
        icon: const Icon(Icons.arrow_back_ios, color: Colors.white, size: 20),
        onPressed: _closeSheet,
        style: IconButton.styleFrom(
          padding: const EdgeInsets.all(8),
          minimumSize: const Size(40, 40),
        ),
      ),
      actions: [
        OutlinedButton(
          onPressed: _onSkip,
          style: OutlinedButton.styleFrom(
            foregroundColor: Colors.white,
            side: const BorderSide(color: Color(0xFFB5B5B5)),
            padding: const EdgeInsets.symmetric(vertical: 12),
          ),
          child: Text(context.tr("permission_sheet.skip")),
        ),
        FilledButton(
          onPressed: _onEnableAll,
          style: FilledButton.styleFrom(
            backgroundColor: StyleConstants.defaultColor,
            foregroundColor: Colors.black,
            padding: const EdgeInsets.symmetric(vertical: 12),
          ),
          child: Text(context.tr("permission_sheet.enable_all")),
        ),
      ],
      child: Column(
        children: [
          _PermissionTile(
            icon: Icons.location_on,
            title: context.tr("permission_sheet.location_title"),
            description: context.tr("permission_sheet.location_desc"),
            status: _location,
            onTap: _requestLocation,
            onRationaleTap: _showLocationRationaleDialog,
            rationaleTooltip:
                context.tr("permission_sheet.location_help_tooltip"),
          ),
          if (Platform.isAndroid)
            _PermissionTile(
              icon: Icons.battery_charging_full,
              title: context.tr("permission_sheet.battery_title"),
              description: context.tr("permission_sheet.battery_desc"),
              status: _battery,
              onTap: _requestBattery,
              onRationaleTap: _showBatteryRationaleDialog,
              rationaleTooltip:
                  context.tr("permission_sheet.battery_help_tooltip"),
            ),
          _PermissionTile(
            icon: Icons.notifications_outlined,
            title: context.tr("permission_sheet.notification_title"),
            description: context.tr("permission_sheet.notification_desc"),
            status: _notification,
            onTap: _requestNotification,
            onRationaleTap: _showNotificationRationaleDialog,
            rationaleTooltip:
                context.tr("permission_sheet.notification_help_tooltip"),
          ),
        ],
      ),
    );
  }
}

class _PermissionTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final String description;
  final PermissionTileStatus status;
  final VoidCallback onTap;
  final VoidCallback? onRationaleTap;
  final String? rationaleTooltip;

  const _PermissionTile({
    required this.icon,
    required this.title,
    required this.description,
    required this.status,
    required this.onTap,
    this.onRationaleTap,
    this.rationaleTooltip,
  });

  @override
  Widget build(BuildContext context) {
    return SetupTile(
      icon: icon,
      title: title,
      subtitle: description,
      onTap: onTap,
      titleTrailing: onRationaleTap == null
          ? null
          : _PermissionInfoIcon(
              onTap: onRationaleTap!,
              tooltip: rationaleTooltip,
            ),
      extraContent: status.permanentlyDenied
          ? Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: TextButton(
                  onPressed: onTap,
                  style: TextButton.styleFrom(
                    padding: EdgeInsets.zero,
                    minimumSize: Size.zero,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    foregroundColor: StyleConstants.defaultColor,
                  ),
                  child: Text(
                    context.tr('permission_sheet.open_system_settings'),
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      decoration: TextDecoration.underline,
                      decorationColor: StyleConstants.defaultColor,
                    ),
                  ),
                ),
              ),
            )
          : null,
      trailing: _PermissionStatusIndicator(
        isGranted: status.granted,
        isDenied: status.denied || status.permanentlyDenied,
        onTap: status.granted ? null : onTap,
      ),
    );
  }
}

class _PermissionStatusIndicator extends StatelessWidget {
  final bool isGranted;
  final bool isDenied;
  final VoidCallback? onTap;

  const _PermissionStatusIndicator({
    required this.isGranted,
    required this.isDenied,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    if (isGranted) {
      return const SizedBox(
        width: 52,
        height: 40,
        child: Center(
          child: Icon(
            Icons.check_circle,
            color: StyleConstants.defaultColor,
            size: 24,
          ),
        ),
      );
    }

    if (isDenied) {
      const color = Color(0xFFFF6B6B);
      return SizedBox(
        width: 52,
        height: 40,
        child: Center(
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(999),
            child: Container(
              width: 30,
              height: 30,
              decoration: BoxDecoration(
                color: color.withValues(alpha: 0.12),
                shape: BoxShape.circle,
                border: Border.all(color: color.withValues(alpha: 0.75)),
              ),
              alignment: Alignment.center,
              child: const Icon(
                Icons.close,
                color: color,
                size: 18,
              ),
            ),
          ),
        ),
      );
    }

    final label = context.tr('permission_sheet.allow');

    return ConstrainedBox(
      constraints: const BoxConstraints(
        minWidth: 52,
        minHeight: 40,
      ),
      child: Center(
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(999),
          child: Container(
            height: 30,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              color: StyleConstants.defaultColor.withValues(alpha: 0.12),
              borderRadius: BorderRadius.circular(999),
              border: Border.all(
                color: StyleConstants.defaultColor.withValues(alpha: 0.75),
              ),
            ),
            alignment: Alignment.center,
            child: Text(
              label,
              style: const TextStyle(
                color: StyleConstants.defaultColor,
                fontSize: 12,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _PermissionInfoIcon extends StatelessWidget {
  const _PermissionInfoIcon({
    required this.onTap,
    this.tooltip,
  });

  final VoidCallback onTap;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    Widget icon = GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: const Icon(
        Icons.info_outline,
        size: 18.0,
        color: Color(0x99FFFFFF),
      ),
    );
    if (tooltip != null && tooltip!.isNotEmpty) {
      icon = Tooltip(message: tooltip!, child: icon);
    }
    return icon;
  }
}
