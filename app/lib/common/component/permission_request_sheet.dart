import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:permission_handler/permission_handler.dart';

/// Shows the unified permission request bottom sheet.
/// Returns true when the user continues (e.g. Skip or Enable all); some permissions may still be denied.
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
    extends State<_PermissionRequestSheetContent> {
  bool _locationGranted = false;
  bool _batteryGranted = false;
  bool _notificationGranted = false;

  bool _locationPermanentlyDenied = false;

  @override
  void initState() {
    super.initState();
    _refreshStatus();
  }

  Future<void> _refreshStatus() async {
    final locStatus = await Permission.location.status;
    final locAlwaysStatus = await Permission.locationAlways.status;
    final isAndroid = defaultTargetPlatform == TargetPlatform.android;
    final batteryGranted = !isAndroid ||
        await Permission.ignoreBatteryOptimizations.isGranted;
    final notificationGranted = await Permission.notification.status.isGranted;
    final hasLocation = locStatus.isGranted || locAlwaysStatus.isGranted;

    if (mounted) {
      setState(() {
        _locationGranted = hasLocation;
        _locationPermanentlyDenied = locStatus.isPermanentlyDenied;
        _batteryGranted = batteryGranted;
        _notificationGranted = notificationGranted;
      });
    }
  }

  Future<void> _requestLocation() async {
    if (_locationPermanentlyDenied) {
      await openAppSettings();
      return;
    }

    if (!await Geolocator.isLocationServiceEnabled()) {
      await Geolocator.openLocationSettings();
      return;
    }

    var status = await Permission.location.status;
    if (status.isPermanentlyDenied) {
      await openAppSettings();
      return;
    }

    if (!status.isGranted) {
      status = await Permission.location.request();
    }

    if (status.isGranted) {
      // iOS: second system prompt for background-capable location; Android skips.
      if (defaultTargetPlatform == TargetPlatform.iOS) {
        await Permission.locationAlways.request();
      }
      if (mounted) {
        setState(() => _locationGranted = true);
      }
    } else if (status.isPermanentlyDenied) {
      if (mounted) {
        setState(() => _locationPermanentlyDenied = true);
      }
    }

    await _refreshStatus();
  }

  Future<void> _requestBattery() async {
    if (defaultTargetPlatform != TargetPlatform.android) return;
    if (_batteryGranted) return;

    final result = await Permission.ignoreBatteryOptimizations.request();
    MMKVUtil.putBool(MMKVKey.requestedBatteryOptimization, true);
    if (mounted) {
      setState(() => _batteryGranted = result.isGranted);
    }
  }

  Future<void> _requestNotification() async {
    if (_notificationGranted) return;

    final result = await Permission.notification.request();
    MMKVUtil.putBool(MMKVKey.isUnexpectedExitNotificationEnabled, result.isGranted);
    MMKVUtil.putBool(MMKVKey.requestedNotification, true);
    if (mounted) {
      setState(() => _notificationGranted = result.isGranted);
    }
  }

  void _onSkip() {
    Navigator.of(context).pop(true);
  }

  Future<void> _onEnableAll() async {
    await _refreshStatus();
    if (!mounted) return;
    if (!_locationGranted) await _requestLocation();
    if (!mounted) return;
    if (defaultTargetPlatform == TargetPlatform.android && !_batteryGranted) {
      await _requestBattery();
    }
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
                  icon: const Icon(Icons.arrow_back_ios, color: Colors.white, size: 20),
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
                    permanentlyDenied: _locationPermanentlyDenied,
                  ),
                  if (defaultTargetPlatform == TargetPlatform.android)
                    _PermissionTile(
                      icon: Icons.battery_charging_full,
                      title: context.tr("permission_sheet.battery_title"),
                      description: context.tr("permission_sheet.battery_desc"),
                      isGranted: _batteryGranted,
                      onTap: _requestBattery,
                    ),
                  _PermissionTile(
                    icon: Icons.notifications_outlined,
                    title: context.tr("permission_sheet.notification_title"),
                    description: context.tr("permission_sheet.notification_desc"),
                    isGranted: _notificationGranted,
                    onTap: _requestNotification,
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
  final bool permanentlyDenied;

  const _PermissionTile({
    required this.icon,
    required this.title,
    required this.description,
    required this.isGranted,
    required this.onTap,
    this.permanentlyDenied = false,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: InkWell(
        onTap: permanentlyDenied ? null : onTap,
        borderRadius: BorderRadius.circular(10),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            color: const Color(0x1AFFFFFF),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Row(
            children: [
              Icon(icon, color: StyleConstants.defaultColor, size: 22),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      title,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      description,
                      style: const TextStyle(
                        color: Color(0xFFB0B0B0),
                        fontSize: 12,
                      ),
                    ),
                  ],
                ),
              ),
              Switch(
                value: isGranted,
                onChanged: permanentlyDenied ? null : (_) => onTap(),
                activeTrackColor: StyleConstants.defaultColor,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
