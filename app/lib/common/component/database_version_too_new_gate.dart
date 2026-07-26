import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/app_bootstrap.dart';
import 'package:url_launcher/url_launcher_string.dart';

const _updateWebsiteUrl = 'https://app.memolanes.com/';

/// Prevents access to the application when its local database requires a
/// newer major schema version than this build supports.
class DatabaseVersionTooNewGate extends StatefulWidget {
  const DatabaseVersionTooNewGate({super.key});

  @override
  State<DatabaseVersionTooNewGate> createState() =>
      _DatabaseVersionTooNewGateState();
}

class _DatabaseVersionTooNewGateState extends State<DatabaseVersionTooNewGate> {
  bool get _isIOS => defaultTargetPlatform == TargetPlatform.iOS;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _showDialog());
  }

  Future<void> _handleAction() async {
    if (_isIOS) {
      await launchUrlString(
        _updateWebsiteUrl,
        mode: LaunchMode.externalApplication,
      );
      return;
    }

    await SystemNavigator.pop();
  }

  Future<void> _showDialog() async {
    if (!mounted) return;
    await AppBootstrap.applyInitialLocale(context);
    if (!mounted) return;

    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (dialogContext) => PopScope(
        canPop: false,
        child: AlertDialog(
          title: Text(dialogContext.tr('startup_error.title')),
          content:
              Text(dialogContext.tr('startup_error.database_version_too_new')),
          actions: [
            TextButton(
              onPressed: _handleAction,
              child: Text(dialogContext.tr(
                _isIOS ? 'startup_error.update' : 'startup_error.exit',
              )),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return const PopScope(
      canPop: false,
      child: Scaffold(body: SizedBox.expand()),
    );
  }
}
