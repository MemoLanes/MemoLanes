import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/recording_health_alert.dart';

class RecordingHealthService {
  static final RecordingHealthService instance = RecordingHealthService._();

  RecordingHealthService._();

  static const _heartbeatInterval = Duration(seconds: 10);
  static const _gapThreshold = Duration(seconds: 60);

  Timer? _heartbeatTimer;
  DateTime? _lastHeartbeatAt;

  bool get isRunning => _heartbeatTimer != null;

  void handleRecordingStatus(GpsRecordingStatus status) {
    if (defaultTargetPlatform != TargetPlatform.android) return;
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

  void _startHeartbeat() {
    if (_heartbeatTimer != null) return;

    _lastHeartbeatAt = DateTime.now();
    _heartbeatTimer = Timer.periodic(_heartbeatInterval, (_) {
      _checkHeartbeatGap();
    });
  }

  void _stopHeartbeat() {
    if (_heartbeatTimer == null) return;

    // A recording status update can arrive immediately after the app resumes,
    // before the next periodic heartbeat. Check the elapsed time before
    // clearing it so a freeze is not missed when recording is stopped then.
    _checkHeartbeatGap();
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
        unawaited(RecordingHealthAlert.instance.showFreezeWarning());
      }
    }
    _lastHeartbeatAt = now;
  }
}
