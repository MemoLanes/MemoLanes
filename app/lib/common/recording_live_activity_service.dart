import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:live_activities/live_activities.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/recording_live_activity_constants.dart';
import 'package:memolanes/common/service/location/location_service.dart';

/// iOS Live Activity + Dynamic Island for journey recording.
class RecordingLiveActivityService with WidgetsBindingObserver {
  RecordingLiveActivityService._();
  static final RecordingLiveActivityService instance =
      RecordingLiveActivityService._();

  final LiveActivities _live = LiveActivities();
  GpsManager? _gps;
  Timer? _coalesce;
  GpsRecordingStatus? _lastObservedStatus;
  DateTime _lastSlowFieldPush = DateTime.fromMillisecondsSinceEpoch(0);
  Map<String, dynamic>? _lastSentPayload;
  bool _hasCreatedTrackedActivity = false;
  bool _inited = false;
  bool _nativeReady = false;

  Future<void> _syncChain = Future<void>.value();

  static const _slowThrottle = Duration(seconds: 5);

  void start({required GpsManager gpsManager}) {
    if (!Platform.isIOS) return;
    _gps = gpsManager;
    _lastObservedStatus = gpsManager.recordingStatus;
    WidgetsBinding.instance.addObserver(this);
    gpsManager.addListener(_onGpsManagerChanged);
    unawaited(_initNativeThenSync());
  }

  Future<void> _initNativeThenSync() async {
    if (_inited) return;
    _inited = true;
    final supported = await _live.areActivitiesSupported();
    final enabled = await _live.areActivitiesEnabled();
    if (!supported || !enabled) {
      log.info(
        '[RecordingLiveActivity] skipped (supported=$supported enabled=$enabled)',
      );
      return;
    }
    await _live.init(
      appGroupId: kRecordingLiveActivityAppGroupId,
      requestAndroidNotificationPermission: false,
    );
    _nativeReady = true;
    await syncNow();
  }

  void _onGpsManagerChanged() {
    final gps = _gps;
    if (gps == null) return;

    final statusChanged = gps.recordingStatus != _lastObservedStatus;
    _lastObservedStatus = gps.recordingStatus;

    if (statusChanged) {
      _coalesce?.cancel();
      unawaited(syncNow());
      return;
    }

    _coalesce?.cancel();
    _coalesce = Timer(const Duration(milliseconds: 80), () {
      unawaited(syncNow());
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    switch (state) {
      case AppLifecycleState.resumed:
        unawaited(syncNow());
      case AppLifecycleState.detached:
        unawaited(_endAllActivities());
      case AppLifecycleState.inactive:
      case AppLifecycleState.hidden:
      case AppLifecycleState.paused:
        break;
    }
  }

  Future<Map<String, dynamic>> _buildPayload(GpsManager gps) async {
    final pos = gps.latestPosition;
    final hasFix = gps.recordingStatus == GpsRecordingStatus.recording &&
        pos != null &&
        !_positionTooOldForLiveActivity(pos);

    return <String, dynamic>{
      'recordingStatus': gps.recordingStatus.index,
      'hasGpsFix': hasFix,
      if (pos != null && hasFix) 'latitude': pos.latitude,
      if (pos != null && hasFix) 'longitude': pos.longitude,
      if (pos != null && hasFix) 'accuracyM': pos.accuracy,
      if (pos != null && hasFix) 'gpsTimestampMs': pos.timestampMs,
    };
  }

  bool _positionTooOldForLiveActivity(LocationData data,
      {int staleThresholdMs = 12 * 1000}) {
    final now = DateTime.now().millisecondsSinceEpoch;
    return now - data.timestampMs >= staleThresholdMs;
  }

  bool _shouldShow(GpsManager gps) =>
      gps.recordingStatus != GpsRecordingStatus.none;

  bool _payloadCanonicallyEqual(
    Map<String, dynamic> a,
    Map<String, dynamic> b,
  ) =>
      jsonEncode(a) == jsonEncode(b);

  Future<void> syncNow() async {
    final gps = _gps;
    if (!_nativeReady || gps == null) return;

    final previous = _syncChain;
    final current = Completer<void>();
    _syncChain = current.future;
    try {
      await previous;
      await _syncBody();
    } finally {
      current.complete();
    }
  }

  Future<void> _syncBody() async {
    final gps = _gps;
    if (gps == null) return;

    if (!_shouldShow(gps)) {
      // End every Live Activity of this attribute type so duplicates from races
      // or older builds cannot linger on the lock screen / 灵动岛.
      await _endAllActivities();
      return;
    }

    final next = await _buildPayload(gps);
    final prev = _lastSentPayload;
    final now = DateTime.now();

    if (prev != null && _payloadCanonicallyEqual(next, prev)) {
      return;
    }

    final statusChanged =
        prev == null || next['recordingStatus'] != prev['recordingStatus'];
    final slowOnly = prev != null &&
        next['recordingStatus'] == prev['recordingStatus'] &&
        (next['hasGpsFix'] != prev['hasGpsFix'] ||
            next['latitude'] != prev['latitude'] ||
            next['longitude'] != prev['longitude'] ||
            next['accuracyM'] != prev['accuracyM'] ||
            next['gpsTimestampMs'] != prev['gpsTimestampMs']);

    if (!statusChanged &&
        slowOnly &&
        now.difference(_lastSlowFieldPush) < _slowThrottle) {
      return;
    }

    final ids = await _live.getAllActivitiesIds();
    if (ids.length > 1 || (ids.isNotEmpty && !_hasCreatedTrackedActivity)) {
      await _endAllActivities();
    }

    await _live.createOrUpdateActivity(
      kRecordingLiveActivityId,
      next,
      removeWhenAppIsKilled: true,
      iOSEnableRemoteUpdates: false,
    );
    _hasCreatedTrackedActivity = true;
    _lastSentPayload = Map<String, dynamic>.from(next);
    _lastSlowFieldPush = now;
  }

  Future<void> _endAllActivities() async {
    if (!_nativeReady) return;
    await _live.endAllActivities();
    _hasCreatedTrackedActivity = false;
    _lastSentPayload = null;
    _lastSlowFieldPush = DateTime.fromMillisecondsSinceEpoch(0);
  }

  void dispose() {
    _coalesce?.cancel();
    _gps?.removeListener(_onGpsManagerChanged);
    WidgetsBinding.instance.removeObserver(this);
  }
}
