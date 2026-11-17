import 'dart:async';
import 'dart:collection';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:mutex/mutex.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:tracelet/tracelet.dart' as tl;

typedef TraceletMotionFix = ({
  DateTime receivedAt,
  int timestampMs,
  int? fixIntervalMs,
  int? callbackIntervalMs,
  int fixAgeMs,
  double accuracy,
  double? speed,
  String source,
  bool isMock,
  bool reducedAccuracy,
});

// Capture belongs to the app, so closing the diagnostics page keeps it running.
class TraceletMotionDiagnostics extends ChangeNotifier
    with WidgetsBindingObserver {
  TraceletMotionDiagnostics(this._documents);

  static final instance = TraceletMotionDiagnostics(
    getApplicationDocumentsDirectory,
  );
  static const _nativeChannel = MethodChannel(
    'com.memolanes/tracelet_motion_diagnostics',
  );
  static bool get _supportsNativeMotion =>
      !kIsWeb &&
      (defaultTargetPlatform == TargetPlatform.iOS ||
          defaultTargetPlatform == TargetPlatform.android);

  final Future<Directory> Function() _documents;
  final _clock = Stopwatch()..start();
  final _captureMutex = Mutex();
  final _recentLines = ListQueue<String>();
  final _subscriptions = <StreamSubscription<dynamic>>[];
  bool _observing = false;
  bool _pageOpen = false;
  Future<void> _writes = Future.value();
  File? _capture;
  Object? _writeError;
  int _captureStartMs = 0;
  StreamSubscription<tl.SpeedMotionEvent>? _speedMotionSubscription;

  bool _recording = false;
  bool? _isMoving;
  int _motionRevision = 0;
  TraceletMotionFix? _lastFix;
  int? _lastCallbackMs;
  int _fixCount = 0;

  bool? get isMoving => _isMoving;
  TraceletMotionFix? get lastFix => _lastFix;
  int get fixCount => _fixCount;
  bool get capturing => _capture != null;
  Object? get writeError => _writeError;
  List<String> get recentLines => _recentLines.toList(growable: false);

  Future<Directory> get _logDirectory async =>
      Directory('${(await _documents()).path}/tracelet_motion_logs');

  Future<void> attach() => _captureMutex.protect(() async {
    _pageOpen = true;
    await _startObserving();
  });

  Future<void> detach() => _captureMutex.protect(() async {
    _pageOpen = false;
    if (!capturing) await _stopObserving();
  });

  Future<void> _startObserving() async {
    if (_observing || !kDebugMode || !_supportsNativeMotion) return;
    _observing = true;
    void reportError(Object error, StackTrace stackTrace) {
      log.error(
        '[Tracelet motion] SDK event stream failed: $error',
        stackTrace,
      );
    }

    try {
      // Observe the SDK directly so diagnostics need no recording-path hooks.
      _subscriptions.add(
        tl.Tracelet.locationStream.listen(_onLocation, onError: reportError),
      );
      _subscriptions.add(
        tl.Tracelet.motionChangeStream.listen(
          (location) => motionChanged(location.isMoving),
          onError: reportError,
        ),
      );
      _subscriptions.add(
        tl.Tracelet.enabledChangeStream.listen((enabled) {
          _motionRevision++;
          if (!enabled) {
            recordingStopped();
          } else {
            if (!_recording) recordingStarted(null);
            unawaited(
              refreshMotionState().catchError((
                Object error,
                StackTrace stackTrace,
              ) {
                log.error(
                  '[Tracelet motion] Refreshing SDK state failed: $error',
                  stackTrace,
                );
              }),
            );
          }
        }, onError: reportError),
      );
      await refreshMotionState();
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet motion] Attaching SDK diagnostics failed: $error',
        stackTrace,
      );
      await _stopObserving();
      rethrow;
    }
  }

  Future<void> _stopObserving() async {
    _observing = false;
    _motionRevision++;
    final subscriptions = _subscriptions.toList();
    _subscriptions.clear();
    for (final subscription in subscriptions) {
      try {
        await subscription.cancel();
      } catch (error, stackTrace) {
        log.error(
          '[Tracelet motion] Cancelling SDK diagnostics failed: $error',
          stackTrace,
        );
      }
    }
    recordingStopped();
  }

  void _onLocation(tl.Location location) {
    if (!_recording) return;
    try {
      final coords = location.coords;
      final timestampMs = DateTime.tryParse(location.timestamp)
          ?.millisecondsSinceEpoch;
      if (timestampMs == null ||
          timestampMs <= 0 ||
          !coords.latitude.isFinite ||
          coords.latitude.abs() > 90 ||
          !coords.longitude.isFinite ||
          coords.longitude.abs() > 180 ||
          !coords.accuracy.isFinite ||
          coords.accuracy < 0) {
        throw const FormatException('Tracelet location has invalid fix data');
      }
      traceletLocationReceived(
        LocationData(
          latitude: coords.latitude,
          longitude: coords.longitude,
          timestampMs: timestampMs,
          accuracy: coords.accuracy,
          altitude: coords.altitude.isFinite ? coords.altitude : null,
          speed: coords.speed.isFinite && coords.speed >= 0
              ? coords.speed
              : null,
        ),
        location,
      );
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet motion] Reading a location event failed: $error',
        stackTrace,
      );
    }
  }

  Future<List<File>> savedLogs() => _captureMutex.protect(() async {
    final directory = await _logDirectory;
    if (!await directory.exists()) return [];
    final files = await directory
        .list(followLinks: false)
        .where(
          (entry) =>
              entry is File &&
              entry.path != _capture?.path &&
              entry.path.endsWith('.jsonl'),
        )
        .cast<File>()
        .toList();
    return files..sort((a, b) => b.path.compareTo(a.path));
  });

  Future<void> startCapture() => _captureMutex.protect(() async {
    if (capturing) return;
    try {
      await _startObserving();
      final directory = await (await _logDirectory).create(recursive: true);
      final info = await PackageInfo.fromPlatform();
      final stamp = DateTime.now().toUtc().toIso8601String().replaceAll(
        ':',
        '-',
      );
      final file = await File('${directory.path}/tracelet-motion-$stamp.jsonl')
          .create(exclusive: true);
      _captureStartMs = _clock.elapsedMilliseconds;
      final header = jsonEncode(
        _row('capture_started', {
          'schema': 2,
          'platform': Platform.operatingSystem,
          'osVersion': Platform.operatingSystemVersion,
          'version': info.version,
          'build': info.buildNumber,
          'isMoving': _isMoving,
          'lifecycle': WidgetsBinding.instance.lifecycleState?.name,
          if (_recording) 'motionConfig': _motionConfig(),
          if (_recording && _supportsNativeMotion)
            'nativeMotion': await _readNativeMotion(),
        }),
      );
      await file.writeAsString('$header\n', flush: true);
      _writeError = null;
      _capture = file;
      _recentLines
        ..clear()
        ..add(header);
      WidgetsBinding.instance.addObserver(this);
      _listenToSpeedMotion();
      notifyListeners();
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet motion] Starting capture failed: $error',
        stackTrace,
      );
      if (!_pageOpen) await _stopObserving();
      rethrow;
    }
  });

  Future<void> stopCapture() => _captureMutex.protect(() async {
    if (!capturing) return;
    _record('capture_stopped', const {});
    _capture = null;
    WidgetsBinding.instance.removeObserver(this);
    final subscription = _speedMotionSubscription;
    _speedMotionSubscription = null;
    try {
      await subscription?.cancel();
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet motion] Could not cancel the speed state stream: $error',
        stackTrace,
      );
    }
    await _writes;
    if (!_pageOpen) await _stopObserving();
    notifyListeners();
  });

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _record('app_lifecycle', {'state': state.name});
  }

  void recordingStarted(bool? isMoving) {
    _recording = true;
    _setMotion(isMoving, 'recording_started', {
      'motionConfig': _motionConfig(),
    });
  }

  void recordingStopped() {
    if (!_recording) return;
    _recording = false;
    _lastFix = null;
    _lastCallbackMs = null;
    _fixCount = 0;
    _setMotion(null, 'recording_stopped');
  }

  void motionChanged(bool isMoving) {
    if (_recording && _isMoving != isMoving) {
      _setMotion(isMoving, 'motion_changed');
    }
  }

  Future<void> refreshMotionState() async {
    if (!_observing) return;
    final revision = _motionRevision;
    final state = await tl.Tracelet.getState();
    if (!_observing || revision != _motionRevision) return;
    if (!state.enabled) {
      recordingStopped();
    } else if (!_recording) {
      recordingStarted(state.isMoving);
    } else if (state.isMoving != _isMoving) {
      _setMotion(state.isMoving, 'motion_snapshot');
    }
  }

  void traceletLocationReceived(LocationData data, tl.Location location) {
    final speedAccuracy = location.coords.speedAccuracy;
    locationReceived(
      data,
      isMovingAtFix: location.isMoving,
      source: location.locationSource,
      isMock: location.isMock,
      reducedAccuracy: location.reducedAccuracy,
      speedAccuracyMps: speedAccuracy.isFinite && speedAccuracy >= 0
          ? speedAccuracy
          : null,
      locationEvent: location.event,
    );
  }

  void locationReceived(
    LocationData location, {
    required bool isMovingAtFix,
    required String source,
    required bool isMock,
    required bool reducedAccuracy,
    double? speedAccuracyMps,
    String? locationEvent,
  }) {
    if (!_recording) return;
    final now = _clock.elapsedMilliseconds;
    final previousCallbackMs = _lastCallbackMs;
    final previousFix = _lastFix;
    final receivedAt = DateTime.now();
    final fix = (
      receivedAt: receivedAt,
      timestampMs: location.timestampMs,
      fixIntervalMs: previousFix == null
          ? null
          : location.timestampMs - previousFix.timestampMs,
      callbackIntervalMs: previousCallbackMs == null
          ? null
          : now - previousCallbackMs,
      fixAgeMs: receivedAt.millisecondsSinceEpoch - location.timestampMs,
      accuracy: location.accuracy,
      speed: location.speed,
      source: source,
      isMock: isMock,
      reducedAccuracy: reducedAccuracy,
    );
    _lastCallbackMs = now;
    _lastFix = fix;
    _fixCount++;
    _record('location', {
      'count': _fixCount,
      'isMovingAtFix': isMovingAtFix,
      'fixAtMs': fix.timestampMs,
      'receivedAtMs': fix.receivedAt.millisecondsSinceEpoch,
      'fixIntervalMs': fix.fixIntervalMs,
      'callbackIntervalMs': fix.callbackIntervalMs,
      'fixAgeMs': fix.fixAgeMs,
      'speedMps': fix.speed,
      'speedAccuracyMps': speedAccuracyMps,
      'locationEvent': locationEvent,
      'accuracyM': fix.accuracy,
      'source': fix.source,
      'isMock': fix.isMock,
      'reducedAccuracy': fix.reducedAccuracy,
    });
    notifyListeners();
  }

  void _setMotion(
    bool? isMoving,
    String type, [
    Map<String, Object?> details = const {},
  ]) {
    _isMoving = isMoving;
    _motionRevision++;
    _record(type, {'isMoving': isMoving, ...details});
    notifyListeners();
  }

  static Map<String, Object?> _motionConfig() {
    final config = tl.Tracelet.activeConfig;
    return {
      'mode': config.motion.motionDetectionMode.name,
      'stopTimeoutMinutes': config.motion.stopTimeout,
      'speedStationaryDelaySeconds': config.motion.speedStationaryDelay,
      'stationaryTrackingMode': config.motion.stationaryTrackingMode.name,
      'stationaryAccuracy': config.motion.stationaryPeriodicAccuracy.name,
      'speedMovingThresholdMps': config.motion.speedMovingThreshold,
      'speedWakeConfirmCount': config.motion.speedWakeConfirmCount,
      'androidLocationUpdateIntervalMs': config.android.locationUpdateInterval,
      'androidStationaryLocationUpdateIntervalMs':
          config.android.stationaryLocationUpdateInterval,
      'androidReleaseWakelockWhenStationary':
          config.android.releaseWakelockWhenStationary,
      if (config.motion.stationaryTrackingMode ==
          tl.StationaryTrackingMode.periodic) ...{
        'periodicLocationIntervalSeconds': config.geo.periodicLocationInterval,
        'stationaryPeriodicIntervalSeconds':
            config.motion.stationaryPeriodicInterval,
      },
    };
  }

  Map<String, Object?> _row(String type, Map<String, Object?> data) => {
    'type': type,
    'wallMs': DateTime.now().millisecondsSinceEpoch,
    'elapsedMs': _clock.elapsedMilliseconds - _captureStartMs,
    ...data,
  };

  void _record(String type, Map<String, Object?> data) {
    final file = _capture;
    if (file == null || _writeError != null) return;
    final row = _row(type, data);
    // Request the native sample before waiting for earlier disk writes.
    final nativeMotion = _recording && _supportsNativeMotion
        ? _readNativeMotion(
            fixAtMs: data['fixAtMs'] as int?,
            speedMps: data['speedMps'] as double?,
          )
        : null;
    _writes = _writes.then((_) async {
      if (_writeError != null) return;
      try {
        if (nativeMotion != null) row['nativeMotion'] = await nativeMotion;
        final line = jsonEncode(row);
        await file.writeAsString('$line\n', mode: FileMode.append, flush: true);
        if (_recentLines.length == 20) _recentLines.removeFirst();
        _recentLines.addLast(line);
      } catch (error, stackTrace) {
        _writeError = error;
        log.error(
          '[Tracelet motion] Could not write $type to '
          '${file.uri.pathSegments.last}: $error\n$stackTrace',
        );
      }
      notifyListeners();
    });
  }

  void _listenToSpeedMotion() {
    if (!_supportsNativeMotion || _speedMotionSubscription != null) return;
    void reportError(Object error, StackTrace stackTrace) {
      log.error(
        '[Tracelet motion] Speed state stream failed: $error\n$stackTrace',
      );
    }

    try {
      _speedMotionSubscription = tl.Tracelet.onSpeedMotionChange((event) {
        if (!_recording) return;
        _record('speed_motion_changed', {
          'previousState': event.previousState.name,
          'speedState': event.state.name,
          'speedTrackingMode': event.trackingMode.name,
        });
      })..onError(reportError);
    } catch (error, stackTrace) {
      reportError(error, stackTrace);
    }
  }

  static Future<Map<String, Object?>> _readNativeMotion({
    int? fixAtMs,
    double? speedMps,
  }) async {
    try {
      final snapshot = await _nativeChannel.invokeMapMethod<String, Object?>(
        'snapshot',
        {'fixAtMs': fixAtMs},
      );
      if (snapshot == null) return {'available': false};
      if (fixAtMs == null) return snapshot;

      final resolvedSpeed = speedMps;
      final rawSpeed = snapshot['rawSpeedMps'] as num?;
      final matched = snapshot['rawFixMatched'] == true;
      String? speedSource;
      if (matched && resolvedSpeed != null) {
        if (rawSpeed != null && rawSpeed >= 0) {
          if (rawSpeed == resolvedSpeed) speedSource = 'platform';
        } else {
          speedSource = resolvedSpeed > 0 ? 'position_delta' : 'zero_fallback';
        }
      }
      return {
        ...snapshot,
        'sdkSpeedMps': resolvedSpeed,
        'sdkSpeedSource': speedSource,
      };
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet motion] Native snapshot failed for $fixAtMs: $error\n$stackTrace',
      );
      return {'available': false};
    }
  }
}
