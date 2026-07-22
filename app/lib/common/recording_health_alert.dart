import 'package:device_info_plus/device_info_plus.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:url_launcher/url_launcher_string.dart';

/// Presents recording-health warnings through the app's root navigator.
///
/// Concurrent requests are deliberately collapsed while the dialog is visible,
/// so a delayed timer callback cannot stack multiple modal routes.
class RecordingHealthAlert {
  RecordingHealthAlert._() : _presentWarning = _showDialog;

  RecordingHealthAlert.forTesting(this._presentWarning);

  static final instance = RecordingHealthAlert._();

  final Future<void> Function(String message) _presentWarning;
  bool _isShowing = false;

  static final _helpPageUrl =
      Uri.parse('https://app.memolanes.com/faqs/android-background-recording');

  bool get isShowing => _isShowing;

  Future<void> showFreezeWarning() {
    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return Future.value();

    return _show(
      () => _showFreezeWarningDialog(
        context,
        context.tr('recording_health.freeze_warning'),
      ),
    );
  }

  Future<void> show(String message) => _show(() => _presentWarning(message));

  Future<void> _show(Future<void> Function() presentWarning) async {
    if (_isShowing) return;

    _isShowing = true;
    try {
      await presentWarning();
    } finally {
      _isShowing = false;
    }
  }

  static Future<void> _showDialog(String message) async {
    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;

    await showCommonDialog(context, message);
  }

  static Future<void> _showFreezeWarningDialog(
    BuildContext context,
    String message,
  ) async {
    final helpUrl = await _helpUrl();
    if (!context.mounted) return;

    final openHelp = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (dialogContext) => CommonDialog(
        title: context.tr('common.info'),
        content: message,
        buttons: [
          DialogButton(
            text: context.tr('recording_health.view_help'),
            onPressed: () => Navigator.of(dialogContext).pop(true),
          ),
          DialogButton(
            text: context.tr('common.ok'),
            onPressed: () => Navigator.of(dialogContext).pop(false),
          ),
        ],
      ),
    );
    if (openHelp == true) {
      await launchUrlString(
        helpUrl.toString(),
        mode: LaunchMode.externalApplication,
      );
    }
  }

  static Future<Uri> _helpUrl() async {
    try {
      final androidInfo = await DeviceInfoPlugin().androidInfo;
      final manufacturer = androidInfo.manufacturer.isNotEmpty
          ? androidInfo.manufacturer
          : androidInfo.brand;
      return helpUrlForBrand(manufacturer);
    } catch (_) {
      return helpUrlForBrand('android');
    }
  }

  static Uri helpUrlForBrand(String brand) {
    final anchor = brand
        .toLowerCase()
        .replaceAll(RegExp(r'[^a-z0-9]+'), '-')
        .replaceAll(RegExp(r'^-|-$'), '');
    return _helpPageUrl.replace(fragment: anchor.isEmpty ? 'android' : anchor);
  }
}
