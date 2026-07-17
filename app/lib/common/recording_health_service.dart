import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';

class RecordingHealthService {
  static final RecordingHealthService instance = RecordingHealthService._();

  RecordingHealthService._();

  static const _heartbeatInterval = Duration(seconds: 10);
  static const _gapThreshold = Duration(seconds: 60);

  Timer? _heartbeatTimer;
  DateTime? _lastHeartbeatAt;
  bool _hasPendingFreezeWarning = false;

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
    _hasPendingFreezeWarning = false;
  }

  bool takePendingFreezeWarning() {
    if (!_hasPendingFreezeWarning) return false;

    _hasPendingFreezeWarning = false;
    return true;
  }

  void restorePendingFreezeWarning() {
    _hasPendingFreezeWarning = true;
  }

  void _startHeartbeat() {
    _heartbeatTimer?.cancel();
    _lastHeartbeatAt = DateTime.now();
    _heartbeatTimer = Timer.periodic(_heartbeatInterval, (_) {
      _checkHeartbeatGap();
    });
  }

  void _stopHeartbeat() {
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
        _hasPendingFreezeWarning = true;
      }
    }
    _lastHeartbeatAt = now;
  }
}
