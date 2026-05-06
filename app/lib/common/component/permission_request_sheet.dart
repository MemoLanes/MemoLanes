import 'dart:io' show Platform;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:permission_handler/permission_handler.dart';

/// Shows the unified permission request bottom sheet (layout + copy only).
///
/// Returns `true` when the user taps **Skip** or **Enable all** (enters the app; permissions may
/// still be incomplete).
///
/// Returns `false` when the user leaves via the leading back button or dismisses the sheet
/// (e.g. swipe down) without choosing Skip / Enable all. Dismissal uses `result ?? false`.
Future<bool> showPermissionRequestSheet(BuildContext context) async {
  final result = await showModalBottomSheet<bool>(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    isDismissible: true,
    builder: (context) {
      return _PermissionRequestSheetContent();
    },
  );
  return result ?? false;
}

class _PermissionRequestSheetContent extends StatefulWidget {
  @override
  State<_PermissionRequestSheetContent> createState() =>
      _PermissionRequestSheetContentState();
}

class _PermissionRequestSheetContentState
    extends State<_PermissionRequestSheetContent> with WidgetsBindingObserver {
  final PermissionService _permissions = PermissionService();

  bool _locationGranted = false;
  bool _batteryGranted = false;
  bool _notificationGranted = false;
  bool _locationPermanentlyDenied = false;
  bool _notificationPermanentlyDenied = false;

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
    _refreshStatus();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _refreshStatus();
    }
  }

  Future<void> _refreshStatus() async {
    final s = await _permissions.readPermissionSnapshot();
    if (!mounted) return;
    setState(() {
      _locationGranted = s.locationTileGranted;
      _locationPermanentlyDenied = s.locationPermanentlyDenied;
      _batteryGranted = s.batteryTileGranted;
      _notificationGranted = s.notificationTileGranted;
      _notificationPermanentlyDenied = s.notificationPermanentlyDenied;
    });
  }

  Future<void> _applyEffects(List<PermissionEffect> effects) async {
    for (final e in effects) {
      if (!mounted) return;
      if (e.messageTrKey != null) {
        await showCommonDialog(context, context.tr(e.messageTrKey!));
      }
      if (!mounted) return;
      if (e.openAppSettings) await openAppSettings();
      if (!mounted) return;
      if (e.openLocationSettings) await Geolocator.openLocationSettings();
    }
  }

  Future<void> _requestLocation() async {
    if (!mounted) return;
    final effects = await _permissions.runLocationRequest();
    await _applyEffects(effects);
    await _refreshStatus();
  }

  Future<void> _requestBattery() async {
    if (!Platform.isAndroid) return;
    if (_batteryGranted) return;
    if (!mounted) return;
    final effects = await _permissions.runBatteryRequest();
    await _applyEffects(effects);
    await _refreshStatus();
  }

  Future<void> _requestNotification() async {
    if (_notificationGranted) return;
    if (!mounted) return;
    final effects = await _permissions.runNotificationRequest();
    await _applyEffects(effects);
    await _refreshStatus();
  }

  void _onSkip() {
    Navigator.of(context).pop(true);
  }

  Future<void> _onEnableAll() async {
    await _refreshStatus();
    if (!mounted) return;
    if (!_locationGranted) await _requestLocation();
    if (!mounted) return;
    if (Platform.isAndroid && !_batteryGranted) await _requestBattery();
    if (!mounted) return;
    if (!_notificationGranted) await _requestNotification();
    if (!mounted) return;
    Navigator.of(context).pop(true);
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.6,
      ),
      decoration: BoxDecoration(
        color: Colors.black,
        borderRadius: const BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: Center(
              child: CustomPaint(
                size: const Size(40.0, 4.0),
                painter: LinePainter(color: const Color(0xFFB5B5B5)),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 0),
            child: Row(
              children: [
                IconButton(
                  icon: const Icon(Icons.arrow_back_ios,
                      color: Colors.white, size: 20),
                  onPressed: () => Navigator.of(context).pop(false),
                  style: IconButton.styleFrom(
                    padding: const EdgeInsets.all(8),
                    minimumSize: const Size(40, 40),
                  ),
                ),
                Expanded(
                  child: Text(
                    context.tr("permission_sheet.title"),
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
                const SizedBox(width: 48),
              ],
            ),
          ),
          Flexible(
            child: SingleChildScrollView(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
              child: Column(
                children: [
                  _PermissionTile(
                    icon: Icons.location_on,
                    title: context.tr("permission_sheet.location_title"),
                    description: context.tr("permission_sheet.location_desc"),
                    isGranted: _locationGranted,
                    onTap: _requestLocation,
                    onRationaleTap: _showLocationRationaleDialog,
                    rationaleTooltip:
                        context.tr("permission_sheet.location_help_tooltip"),
                    showOpenSettingsHintWhenDenied: _locationPermanentlyDenied,
                  ),
                  if (Platform.isAndroid)
                    _PermissionTile(
                      icon: Icons.battery_charging_full,
                      title: context.tr("permission_sheet.battery_title"),
                      description: context.tr("permission_sheet.battery_desc"),
                      isGranted: _batteryGranted,
                      onTap: _requestBattery,
                      onRationaleTap: _showBatteryRationaleDialog,
                      rationaleTooltip:
                          context.tr("permission_sheet.battery_help_tooltip"),
                    ),
                  _PermissionTile(
                    icon: Icons.notifications_outlined,
                    title: context.tr("permission_sheet.notification_title"),
                    description:
                        context.tr("permission_sheet.notification_desc"),
                    isGranted: _notificationGranted,
                    onTap: _requestNotification,
                    onRationaleTap: _showNotificationRationaleDialog,
                    rationaleTooltip: context
                        .tr("permission_sheet.notification_help_tooltip"),
                    showOpenSettingsHintWhenDenied:
                        _notificationPermanentlyDenied,
                  ),
                ],
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 10, 20, 20),
            child: Row(
              children: [
                Expanded(
                  child: OutlinedButton(
                    onPressed: _onSkip,
                    style: OutlinedButton.styleFrom(
                      foregroundColor: Colors.white,
                      side: const BorderSide(color: Color(0xFFB5B5B5)),
                      padding: const EdgeInsets.symmetric(vertical: 12),
                    ),
                    child: Text(context.tr("permission_sheet.skip")),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: FilledButton(
                    onPressed: _onEnableAll,
                    style: FilledButton.styleFrom(
                      backgroundColor: StyleConstants.defaultColor,
                      foregroundColor: Colors.black,
                      padding: const EdgeInsets.symmetric(vertical: 12),
                    ),
                    child: Text(context.tr("permission_sheet.enable_all")),
                  ),
                ),
              ],
            ),
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
  final bool isGranted;
  final VoidCallback onTap;
  final VoidCallback? onRationaleTap;
  final String? rationaleTooltip;
  final bool showOpenSettingsHintWhenDenied;

  const _PermissionTile({
    required this.icon,
    required this.title,
    required this.description,
    required this.isGranted,
    required this.onTap,
    this.onRationaleTap,
    this.rationaleTooltip,
    this.showOpenSettingsHintWhenDenied = false,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: const Color(0x1AFFFFFF),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(8),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Icon(
                  icon,
                  color: StyleConstants.defaultColor,
                  size: 22,
                ),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      Flexible(
                        fit: FlexFit.loose,
                        child: InkWell(
                          onTap: onTap,
                          borderRadius: BorderRadius.circular(8),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(vertical: 2),
                            child: Text(
                              title,
                              style: const TextStyle(
                                color: Colors.white,
                                fontSize: 15,
                                fontWeight: FontWeight.w500,
                              ),
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ),
                      ),
                      if (onRationaleTap != null) ...[
                        const SizedBox(width: 6),
                        _PermissionInfoIcon(
                          onTap: onRationaleTap!,
                          tooltip: rationaleTooltip,
                        ),
                      ],
                    ],
                  ),
                  InkWell(
                    onTap: onTap,
                    borderRadius: BorderRadius.circular(8),
                    child: Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: Text(
                        description,
                        style: const TextStyle(
                          color: Color(0xFFB0B0B0),
                          fontSize: 12,
                        ),
                      ),
                    ),
                  ),
                  if (showOpenSettingsHintWhenDenied && !isGranted) ...[
                    const SizedBox(height: 6),
                    Align(
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
                  ],
                ],
              ),
            ),
            Switch(
              value: isGranted,
              onChanged: isGranted ? null : (_) => onTap(),
              activeTrackColor: StyleConstants.defaultColor,
            ),
          ],
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
