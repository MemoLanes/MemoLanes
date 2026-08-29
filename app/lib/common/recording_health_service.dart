import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/utils/nav_helper.dart';

class RecordingHealthService extends ChangeNotifier {
  static final RecordingHealthService instance = RecordingHealthService._();

  RecordingHealthService._();

  static const _heartbeatInterval = Duration(seconds: 10);
  static const _gapThreshold = Duration(seconds: 60);

  Timer? _heartbeatTimer;
  DateTime? _lastHeartbeatAt;
  final _alert = _RecordingHealthAlert();

  bool get isRunning => _heartbeatTimer != null;
  bool get isHeartbeatDetectionEnabled => MMKVUtil.getBool(
        MMKVKey.isHeartbeatDetectionEnabled,
        defaultValue: true,
      );

  void handleRecordingStatus(GpsRecordingStatus status) {
    if (defaultTargetPlatform != TargetPlatform.android) return;
    if (!isHeartbeatDetectionEnabled) {
      _stopHeartbeat(checkGap: false);
      return;
    }
    switch (status) {
      case GpsRecordingStatus.recording:
        _startHeartbeat();
      case GpsRecordingStatus.none || GpsRecordingStatus.paused:
        _stopHeartbeat();
    }
  }

  void stop() {
    _stopHeartbeat();
  }

  void setHeartbeatDetectionEnabled(
    bool enabled, {
    GpsRecordingStatus? recordingStatus,
  }) {
    if (enabled == isHeartbeatDetectionEnabled) return;

    MMKVUtil.putBool(MMKVKey.isHeartbeatDetectionEnabled, enabled);
    if (!enabled) {
      _stopHeartbeat(checkGap: false);
    } else if (defaultTargetPlatform == TargetPlatform.android &&
        recordingStatus == GpsRecordingStatus.recording) {
      _startHeartbeat();
    }
    notifyListeners();
  }

  void _startHeartbeat() {
    if (_heartbeatTimer != null) return;

    _lastHeartbeatAt = DateTime.now();
    _heartbeatTimer = Timer.periodic(_heartbeatInterval, (_) {
      _checkHeartbeatGap();
    });
  }

  void _stopHeartbeat({bool checkGap = true}) {
    if (_heartbeatTimer == null) return;

    // A recording status update can arrive immediately after the app resumes,
    // before the next periodic heartbeat. Check the elapsed time before
    // clearing it so a freeze is not missed when recording is stopped then.
    if (checkGap) {
      _checkHeartbeatGap();
    }
    _heartbeatTimer?.cancel();
    _heartbeatTimer = null;
    _lastHeartbeatAt = null;
  }

  void _checkHeartbeatGap() {
    final now = DateTime.now();
    final lastHeartbeatAt = _lastHeartbeatAt;
    if (lastHeartbeatAt != null) {
      final gap = now.difference(lastHeartbeatAt);
      if (gap > _gapThreshold) {
        log.warning(
          '[RecordingHealthService] heartbeat gap detected while recording: ${gap.inSeconds}s',
        );
        unawaited(_alert.showFreezeWarning());
      }
    }
    _lastHeartbeatAt = now;
  }
}

class _RecordingHealthAlert {
  static final _helpPageUrl =
      Uri.parse('https://app.memolanes.com/faqs/android-background-recording');

  bool _isShowingWarning = false;

  Future<void> showFreezeWarning() async {
    if (_isShowingWarning) return;

    final context = navigatorKey.currentState?.context;
    if (context == null || !context.mounted) return;

    _isShowingWarning = true;
    try {
      final helpUrl = await _helpUrl();
      if (!context.mounted) return;

      final disableHeartbeatDetection = await showAppDialog<bool>(
        context,
        barrierDismissible: false,
        child: CommonDialog(
          title: context.tr('recording_health.interruption_detected'),
          content: '${context.tr('recording_health.freeze_warning')}\n\n'
              '**[${context.tr('recording_health.view_help')} ↗]($helpUrl)**',
          markdown: true,
          buttons: [
            DialogButton(
              text: context.tr('recording_health.dont_remind_again'),
              variant: AppButtonVariant.danger,
              onPressed: () => Navigator.of(context).pop(true),
            ),
            DialogButton(
              text: context.tr('common.ok'),
              onPressed: () => Navigator.of(context).pop(false),
            ),
          ],
        ),
      );
      if (disableHeartbeatDetection == true) {
        RecordingHealthService.instance.setHeartbeatDetectionEnabled(false);
      }
    } finally {
      _isShowingWarning = false;
    }
  }

  static Future<Uri> _helpUrl() async {
    try {
      final androidInfo = await DeviceInfoPlugin().androidInfo;
      final manufacturer = androidInfo.manufacturer.isNotEmpty
          ? androidInfo.manufacturer
          : androidInfo.brand;
      return _helpUrlForBrand(manufacturer);
    } catch (_) {
      return _helpUrlForBrand('android');
    }
  }

  static Uri _helpUrlForBrand(String brand) {
    final anchor = brand
        .toLowerCase()
        .replaceAll(RegExp(r'[^a-z0-9]+'), '-')
        .replaceAll(RegExp(r'^-|-$'), '');
    return _helpPageUrl.replace(fragment: anchor.isEmpty ? 'android' : anchor);
  }
}
