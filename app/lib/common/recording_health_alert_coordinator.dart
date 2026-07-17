import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:memolanes/common/app_lifecycle_service.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/recording_health_service.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/utils/nav_helper.dart';

class RecordingHealthAlertCoordinator {
  static final RecordingHealthAlertCoordinator instance =
      RecordingHealthAlertCoordinator._();

  RecordingHealthAlertCoordinator._();

  static const _checkDelay = Duration(milliseconds: 300);

  AppLifecycleService _lifecycleService = AppLifecycleService.instance;
  RecordingHealthService _healthService = RecordingHealthService.instance;
  StreamSubscription<AppVisibility>? _visibilitySubscription;
  Timer? _scheduledCheck;
  bool _isShowing = false;
  bool _checkAfterCurrentDialog = false;

  bool get isRunning => _visibilitySubscription != null;

  void start({
    AppLifecycleService? lifecycleService,
    RecordingHealthService? healthService,
  }) {
    if (_visibilitySubscription != null) return;

    _lifecycleService = lifecycleService ?? AppLifecycleService.instance;
    _healthService = healthService ?? RecordingHealthService.instance;
    _visibilitySubscription =
        _lifecycleService.visibilityChanges.listen((visibility) {
      if (visibility == AppVisibility.foreground) {
        _requestCheck();
      }
    });
    _requestCheck();
  }

  void _requestCheck() {
    if (_isShowing) {
      _checkAfterCurrentDialog = true;
      return;
    }

    _scheduledCheck?.cancel();
    _scheduledCheck = Timer(_checkDelay, () {
      _scheduledCheck = null;
      unawaited(_showWarningIfNeeded());
    });
  }

  Future<void> _showWarningIfNeeded() async {
    if (_isShowing || !_lifecycleService.isForeground) return;

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;
    if (!_healthService.takePendingFreezeWarning()) return;

    _isShowing = true;
    try {
      await showCommonDialog(
        context,
        context.tr('recording_health.freeze_warning'),
      );
    } catch (error, stackTrace) {
      _healthService.restorePendingFreezeWarning();
      log.error(
        '[RecordingHealthAlertCoordinator] failed to show warning: $error',
        stackTrace,
      );
    } finally {
      _isShowing = false;
      if (_checkAfterCurrentDialog) {
        _checkAfterCurrentDialog = false;
        _requestCheck();
      }
    }
  }

  void stop() {
    unawaited(_visibilitySubscription?.cancel());
    _visibilitySubscription = null;
    _scheduledCheck?.cancel();
    _scheduledCheck = null;
    _isShowing = false;
    _checkAfterCurrentDialog = false;
  }
}
