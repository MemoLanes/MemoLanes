import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Prevents access to the application when its local database requires a
/// newer major schema version than this build supports.
class DatabaseVersionTooNewGate extends StatefulWidget {
  const DatabaseVersionTooNewGate({super.key});

  @override
  State<DatabaseVersionTooNewGate> createState() =>
      _DatabaseVersionTooNewGateState();
}

class _DatabaseVersionTooNewGateState extends State<DatabaseVersionTooNewGate> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _showDialog());
  }

  Future<void> _showDialog() async {
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
              onPressed: SystemNavigator.pop,
              child: Text(dialogContext.tr('startup_error.exit')),
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
