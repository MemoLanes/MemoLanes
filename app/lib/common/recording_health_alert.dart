import 'package:easy_localization/easy_localization.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/utils/nav_helper.dart';

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

  bool get isShowing => _isShowing;

  Future<void> showFreezeWarning() {
    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return Future.value();

    return show(context.tr('recording_health.freeze_warning'));
  }

  Future<void> show(String message) async {
    if (_isShowing) return;

    _isShowing = true;
    try {
      await _presentWarning(message);
    } finally {
      _isShowing = false;
    }
  }

  static Future<void> _showDialog(String message) async {
    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;

    await showCommonDialog(context, message);
  }
}
