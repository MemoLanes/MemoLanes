import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:live_activities/live_activities.dart';
import 'package:live_activities/models/url_scheme_data.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/home_tab_bridge.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/recording_live_activity_constants.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';

/// iOS Live Activity + Dynamic Island for journey recording (§1 in agent_live_activity_requirements).
class RecordingLiveActivityService with WidgetsBindingObserver {
  RecordingLiveActivityService._();
  static final RecordingLiveActivityService instance =
      RecordingLiveActivityService._();

  final LiveActivities _live = LiveActivities();
  GpsManager? _gps;
  StreamSubscription<UrlSchemeData>? _urlSub;
  Timer? _coalesce;
  DateTime _lastSlowFieldPush = DateTime.fromMillisecondsSinceEpoch(0);
  Map<String, dynamic>? _lastSentPayload;
  bool _inited = false;
  bool _nativeReady = false;

  static const _slowThrottle = Duration(seconds: 5);

  void start({required GpsManager gpsManager}) {
    if (!Platform.isIOS) return;
    _gps = gpsManager;
    WidgetsBinding.instance.addObserver(this);
    gpsManager.addListener(_onGpsManagerChanged);
    unawaited(_initNativeThenSync());
  }

  Future<void> _initNativeThenSync() async {
    if (_inited) return;
    _inited = true;
    try {
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
        urlScheme: kRecordingLiveActivityUrlScheme,
        requestAndroidNotificationPermission: false,
      );
      _urlSub = _live.urlSchemeStream().listen(
        _onUrlScheme,
        onError: (Object e, StackTrace s) =>
            log.error('[RecordingLiveActivity] url scheme stream $e', s),
      );
      _nativeReady = true;
    } catch (e, s) {
      log.error('[RecordingLiveActivity] init failed: $e', s);
      return;
    }
    await syncNow(reason: 'init');
  }

  void _onGpsManagerChanged() {
    _coalesce?.cancel();
    _coalesce = Timer(const Duration(milliseconds: 80), () {
      unawaited(syncNow(reason: 'gps_notify'));
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(syncNow(reason: 'resumed'));
    }
  }

  String? _queryFirst(UrlSchemeData data, String name) {
    for (final item in data.queryParameters) {
      if (item['name'] == name) return item['value'];
    }
    return null;
  }

  Future<void> _onUrlScheme(UrlSchemeData data) async {
    final gps = _gps;
    if (gps == null) return;

    final op = _queryFirst(data, 'op');
    if (op == null || op.isEmpty) {
      HomeTabBridge.openMapTab();
      return;
    }

    switch (op) {
      case 'openMap':
        HomeTabBridge.openMapTab();
        return;
      case 'pause':
        if (gps.recordingStatus == GpsRecordingStatus.recording) {
          await gps.changeRecordingState(GpsRecordingStatus.paused);
        }
        return;
      case 'resume':
        if (gps.recordingStatus == GpsRecordingStatus.paused) {
          await gps.changeRecordingState(GpsRecordingStatus.recording);
        }
        return;
      case 'end':
        final ctx = navigatorKey.currentState?.context;
        if (ctx == null || !ctx.mounted) return;
        final shouldEnd = await showCommonDialog(
          ctx,
          ctx.tr('home.end_journey_message'),
          hasCancel: true,
          title: ctx.tr('home.end_journey_title'),
          confirmButtonText: ctx.tr('common.end'),
          confirmGroundColor: Colors.red,
          confirmTextColor: Colors.white,
        );
        if (shouldEnd == true) {
          await gps.changeRecordingState(GpsRecordingStatus.none);
        }
        return;
      default:
        HomeTabBridge.openMapTab();
    }
  }

  /// §1.3 payload keys: recordingStatus, startedAtEpochMs, accuracyM, hasGpsFix.
  Future<Map<String, dynamic>> _buildPayload(GpsManager gps) async {
    final startedMs = await api.ongoingJourneyStartEpochMs();
    final int? started = startedMs?.toInt();
    final pos = gps.latestPosition;
    final hasFix = pos != null && !_positionTooOldForLiveActivity(pos);

    return <String, dynamic>{
      'recordingStatus': gps.recordingStatus.index,
      if (started != null) 'startedAtEpochMs': started,
      if (pos != null && hasFix) 'accuracyM': pos.accuracy,
      'hasGpsFix': hasFix,
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

  Future<void> syncNow({required String reason}) async {
    final gps = _gps;
    if (!_nativeReady || gps == null) return;

    try {
      if (!_shouldShow(gps)) {
        _lastSentPayload = null;
        _lastSlowFieldPush = DateTime.fromMillisecondsSinceEpoch(0);
        await _live.endActivity(kRecordingLiveActivityId);
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
              next['accuracyM'] != prev['accuracyM'] ||
              next['startedAtEpochMs'] != prev['startedAtEpochMs']);

      if (!statusChanged &&
          slowOnly &&
          now.difference(_lastSlowFieldPush) < _slowThrottle) {
        return;
      }

      await _live.createOrUpdateActivity(
        kRecordingLiveActivityId,
        next,
        iOSEnableRemoteUpdates: false,
      );
      _lastSentPayload = Map<String, dynamic>.from(next);
      _lastSlowFieldPush = now;
    } catch (e, s) {
      log.error('[RecordingLiveActivity] sync ($reason): $e', s);
    }
  }

  void dispose() {
    _coalesce?.cancel();
    _urlSub?.cancel();
    _gps?.removeListener(_onGpsManagerChanged);
    WidgetsBinding.instance.removeObserver(this);
  }
}
