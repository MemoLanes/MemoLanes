import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';

/// Display is best-effort; recording remains owned entirely by GpsManager.
class RecordingLiveActivityService with WidgetsBindingObserver {
  static const _channel = MethodChannel(
    'com.memolanes/recording_live_activity',
  );
  static RecordingLiveActivityService? _instance;

  static void start(GpsManager gpsManager) {
    if (kIsWeb || defaultTargetPlatform != TargetPlatform.iOS) return;
    _instance ??= RecordingLiveActivityService(gpsManager);
    unawaited(_instance!._initialize());
  }

  @visibleForTesting
  RecordingLiveActivityService(
    this._gps, {
    Future<void> Function(Map<String, Object?>)? synchronize,
    DateTime Function()? now,
  }) : _synchronize = synchronize ?? _send,
       _now = now ?? DateTime.now;

  static Future<void> _send(Map<String, Object?> payload) async {
    await _channel.invokeMethod<void>('synchronize', payload);
  }

  final GpsManager _gps;
  final Future<void> Function(Map<String, Object?>) _synchronize;
  final DateTime Function() _now;
  Future<void> _operations = Future<void>.value();
  Future<void>? _initialization;
  Timer? _trailingUpdate;
  Timer? _heartbeat;
  GpsRecordingStatus? _observedStatus;
  bool? _observedFix;
  DateTime? _lastSentAt;
  int _version = 0;
  bool _disposed = false;
  bool _pendingSync = false;
  bool _initialized = false;

  Future<void> _initialize() => _initialization ??= _initializeOnce();

  Future<void> _initializeOnce() async {
    try {
      await _gps.initialized;
      if (_disposed) return;
      _initialized = true;
      _gps.addListener(_changed);
      WidgetsBinding.instance.addObserver(this);
      _changed();
      // This renews waiting-state staleDate only while the process runs.
      // It neither wakes the app nor changes the location recording policy.
      _heartbeat = Timer.periodic(const Duration(seconds: 30), (_) {
        if (_gps.recordingStatus == GpsRecordingStatus.recording ||
            _pendingSync) {
          _enqueue();
        }
      });
    } catch (error, stack) {
      log.error('[LiveActivity] initialization failed: $error', stack);
    }
  }

  @visibleForTesting
  Future<void> initialize() => _initialize();

  Map<String, Object?> _snapshot() {
    final status = _gps.recordingStatus;
    final position = _gps.recordingPosition;
    final now = _now();
    final age = position == null
        ? null
        : now.millisecondsSinceEpoch - position.timestampMs;
    final fresh =
        status == GpsRecordingStatus.recording &&
        position != null &&
        age! >= 0 &&
        age < 12000 &&
        position.accuracy.isFinite &&
        position.accuracy >= 0;
    return {
      'status': status.name,
      // Null explicitly removes any previous precision/time. No coordinates
      // leave the app, even when a fix is valid.
      'accuracy': fresh ? position.accuracy : null,
      'gpsTimestampMs': fresh ? position.timestampMs : null,
    };
  }

  void _changed() {
    if (_disposed) return;
    final status = _gps.recordingStatus;
    final statusChanged = status != _observedStatus;
    if (statusChanged) {
      _observedStatus = status;
      _version++;
    }
    final payload = _snapshot();
    final fresh = payload['gpsTimestampMs'] != null;
    final fixChanged = fresh != _observedFix;
    _observedFix = fresh;

    if (statusChanged || fixChanged) {
      _trailingUpdate?.cancel();
      _trailingUpdate = null;
      _enqueue(payload: payload);
      return;
    }
    if (status == GpsRecordingStatus.none) return;
    final elapsed = _lastSentAt == null
        ? const Duration(seconds: 5)
        : _now().difference(_lastSentAt!);
    if (elapsed >= const Duration(seconds: 5)) {
      _enqueue();
    } else {
      _trailingUpdate ??= Timer(const Duration(seconds: 5) - elapsed, () {
        _trailingUpdate = null;
        _enqueue();
      });
    }
  }

  void _enqueue({Map<String, Object?>? payload}) {
    if (_disposed || !_initialized) return;
    _pendingSync = true;
    final version = _version;
    // Business transitions must all survive the queue (none -> recording
    // starts a new display session). Ordinary GPS work may be obsolete.
    final transition = payload;
    _operations = _operations.then((_) async {
      if (_disposed || (transition == null && version != _version)) return;
      try {
        await _synchronize(transition ?? _snapshot());
        _lastSentAt = _now();
        if (version == _version) _pendingSync = false;
      } catch (error, stack) {
        _pendingSync = true;
        log.error('[LiveActivity] sync failed: $error', stack);
        // Keep pending; the next state/foreground/heartbeat event retries.
      }
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _enqueue();
  }

  /// An AppIntent must not return before its pending display cleanup completes.
  /// Display errors are caught by the queue, so recording's
  /// success or failure stays independent of ActivityKit.
  static Future<void> flush() async {
    await _instance?.flushPending();
  }

  @visibleForTesting
  Future<void> flushPending() async {
    try {
      await _flushPending().timeout(const Duration(seconds: 20));
    } catch (error, stack) {
      // This bounds the AppIntent's wait, not the native operation. It doesn't
      // cancel or resend work already in flight.
      log.error('[LiveActivity] shortcut flush failed: $error', stack);
    }
  }

  Future<void> _flushPending() async {
    await _initialize();
    if (!_initialized) return;
    _enqueue();
    await _operations;
  }

  @visibleForTesting
  Future<void> get idle => _operations;

  void dispose() {
    _disposed = true;
    _gps.removeListener(_changed);
    WidgetsBinding.instance.removeObserver(this);
    _trailingUpdate?.cancel();
    _heartbeat?.cancel();
    // Leaving the foreground isn't stopping; only a confirmed none ends it.
  }
}
