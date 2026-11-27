import 'dart:async';

import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:mutex/mutex.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:tracelet/tracelet.dart' as tl;
import 'package:uuid/uuid.dart';

import 'location_service.dart';

class TraceletLocationService extends ILocationService {
  static const _pendingQueueKey =
      'TraceletLocationService.recordingQueuePending';

  TraceletLocationService(this._recordLocation, {this.checkBatchStart});

  final Future<void> Function(LocationData data) _recordLocation;
  final Future<bool> Function(LocationData data)? checkBatchStart;

  final _locations = StreamController<LocationData>.broadcast();
  final _recordingQueue = Mutex();
  StreamSubscription<tl.Location>? _locationSubscription;
  int? _watchId;
  bool _ready = false;
  bool _recording = false;
  bool _drainScheduled = false;

  @override
  LocationBackend get locationBackend => LocationBackend.tracelet;

  @override
  bool get handlesRecording => true;

  Future<bool> get canResumeRecording =>
      PermissionService().checkLocationPermission();

  bool get _hasPendingRecording => MMKVUtil.getBool(_pendingQueueKey);

  @override
  Future<void> initialize() => _recordingQueue.protect(() async {
    final wasRecording = MMKVUtil.getBool(MMKVKey.isRecording);
    if (wasRecording) _saveQueuePending(true);
    if (!_hasPendingRecording) return;
    _saveRecordingIntent(false);
    final resumeRecording = wasRecording && await canResumeRecording;
    await _configure(false);
    await _writeRecordingQueue();
    _saveQueuePending(false);
    if (resumeRecording) _saveRecordingIntent(true);
  });

  void _saveQueuePending(bool pending) {
    if (!MMKVUtil.putBool(_pendingQueueKey, pending)) {
      throw StateError('Could not save the Tracelet recording queue state');
    }
  }

  void _saveRecordingIntent(bool recording) {
    if (!MMKVUtil.putBool(MMKVKey.isRecording, recording)) {
      throw StateError('Could not save the Tracelet recording state');
    }
  }

  Future<void> _stopPersistedTracking() async {
    if ((await tl.Tracelet.getState()).enabled) await _stopNativeTracking();
  }

  Future<void> _stopNativeTracking() async {
    if ((await tl.Tracelet.stop()).enabled) {
      throw StateError('Tracelet remained enabled after stopping');
    }
    _recording = false;
  }

  @override
  Future<void> startLocationUpdates(bool enableBackground) async {
    if (_locationSubscription != null || _watchId != null) return;

    try {
      if (enableBackground) {
        if (!await canResumeRecording) {
          throw StateError('Tracelet recording requires location access');
        }
        _saveQueuePending(true);
        _saveRecordingIntent(true);
        _locationSubscription = tl.Tracelet.locationStream.listen(
          _onLocation,
          onError: (Object error, StackTrace stackTrace) {
            log.error('[Tracelet] Location stream failed: $error', stackTrace);
          },
        );
      }

      await _configure(enableBackground);
      if (!enableBackground) {
        final watchId = await tl.Tracelet.watchPosition(_onLocation);
        _watchId = watchId;
        if (watchId < 0) throw StateError('Tracelet could not watch positions');
      } else {
        final state = await tl.Tracelet.start();
        if (!state.enabled) throw StateError('Tracelet tracking did not start');
        _recording = true;
        _scheduleRecordingDrain();
      }
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet] Starting location updates failed: $error',
        stackTrace,
      );
      if (enableBackground) {
        try {
          _saveRecordingIntent(false);
        } catch (cleanupError, cleanupStackTrace) {
          log.error(
            '[Tracelet] Saving the failed recording state failed: $cleanupError',
            cleanupStackTrace,
          );
        }
      }
      try {
        await _stopNativeTracking();
      } catch (cleanupError, cleanupStackTrace) {
        log.error(
          '[Tracelet] Could not stop after startup failed: $cleanupError',
          cleanupStackTrace,
        );
      }
      await _cancelListeners();
      rethrow;
    }
  }

  @override
  Future<void> stopLocationUpdates() async {
    final hasPendingRecording = _hasPendingRecording;
    if (_locationSubscription == null &&
        _watchId == null &&
        !hasPendingRecording) {
      return;
    }
    if (_recording || hasPendingRecording) _saveRecordingIntent(false);
    // The iOS SDK leaves Core Location running after stopWatchPosition().
    await _stopNativeTracking();
    await _cancelListeners();
  }

  Future<void> _cancelListeners() async {
    final watchId = _watchId;
    _watchId = null;
    if (watchId != null) {
      try {
        if (!await tl.Tracelet.stopWatchPosition(watchId)) {
          throw StateError('Tracelet could not stop the position watcher');
        }
      } catch (error, stackTrace) {
        log.error(
          '[Tracelet] Position watcher cleanup failed: $error',
          stackTrace,
        );
      }
    }

    final subscription = _locationSubscription;
    _locationSubscription = null;
    try {
      await subscription?.cancel();
    } catch (error, stackTrace) {
      log.error('[Tracelet] Event listener cleanup failed: $error', stackTrace);
    }
  }

  @override
  StreamSubscription<LocationData> onLocationUpdate(
    void Function(LocationData) callback,
  ) => _locations.stream.listen(callback);

  @override
  Future<void> flushRecording() => _recordingQueue.protect(() async {
    if (!_hasPendingRecording) return;
    await _writeRecordingQueue();
    if (!_recording) _saveQueuePending(false);
  });

  void _scheduleRecordingDrain() {
    if (!_recording || _drainScheduled) return;
    _drainScheduled = true;
    unawaited(
      _recordingQueue
          .protect(() async {
            _drainScheduled = false;
            if (_recording) await _writeRecordingQueue();
          })
          .catchError((Object error, StackTrace stackTrace) {
            log.error(
              '[Tracelet] Draining the recording queue failed: $error',
              stackTrace,
            );
          }),
    );
  }

  Future<void> _writeRecordingQueue() async {
    // The SDK returns an empty queue before ready(), even when rows exist.
    if (!_ready) await _configure(false);
    final checkBatchStart = this.checkBatchStart;
    var batchStartChecked = false;
    while (true) {
      final locations = await tl.Tracelet.getLocations(
        const tl.SQLQuery(limit: 100),
      );
      if (locations.isEmpty) return;
      for (final location in locations) {
        final uuid = location.uuid;
        if (!Uuid.isValidUUID(fromString: uuid)) {
          throw const FormatException(
            'Queued Tracelet location has an invalid UUID',
          );
        }
        LocationData? data;
        try {
          data = _toLocationData(location);
        } on FormatException catch (error, stackTrace) {
          log.error(
            '[Tracelet] Discarding an invalid queued fix: $error',
            stackTrace,
          );
        }
        if (data != null) {
          if (!batchStartChecked && checkBatchStart != null) {
            batchStartChecked = await checkBatchStart(data);
          }
          await _recordLocation(data);
        }
        if (!await tl.Tracelet.destroyLocation(uuid)) {
          throw StateError(
            'Tracelet could not remove a processed location from its queue',
          );
        }
      }
    }
  }

  Future<void> _configure(bool enableBackground) async {
    if (!_ready) {
      // On cold launch getState() reports disabled until ready(), which can
      // otherwise resume a session from its saved enabled flag.
      await _stopNativeTracking();
    } else if (!enableBackground) {
      await _stopPersistedTracking();
    }
    final motionAuthorization = enableBackground
        ? await Permission.sensors.status
        : PermissionStatus.denied;
    await tl.Tracelet.ready(
      _config(enableBackground, motionAuthorization.isGranted),
    );
    _ready = true;
    if (enableBackground && !motionAuthorization.isGranted) {
      log.info(
        '[Tracelet] Motion activity unavailable or not authorized '
        '($motionAuthorization); using sensor-based detection',
      );
    }
  }

  tl.Config _config(bool enableBackground, bool enableMotionActivity) {
    final base = tl.Config.balanced();
    const accelerometerStopDelay = Duration(minutes: 2);
    final useSmartPowerSaving =
        enableBackground &&
        MMKVUtil.getBool(MMKVKey.smartPowerSavingEnabled, defaultValue: true);
    return base.copyWith(
      geo: base.geo.copyWith(
        // Preserve route detail at the moving pace.
        desiredAccuracy: tl.DesiredAccuracy.high,
        distanceFilter: 0,
        enableAdaptiveMode: false,
        filter: base.geo.filter.mergedWith(
          // The default ceiling filters out aircraft speeds.
          const tl.LocationFilter(maxImpliedSpeed: 0),
        ),
      ),
      app: base.app.copyWith(
        stopOnTerminate: true,
        startOnBoot: false,
        heartbeatInterval: enableBackground ? 60 : -1,
      ),
      ios: base.ios.copyWith(
        locationAuthorizationRequest: tl.LocationAuthorizationRequest.whenInUse,
        disableLocationAuthorizationAlert: true,
      ),
      motion: base.motion.mergedWith(
        tl.MotionConfig(
          stopTimeout: enableBackground
              ? accelerometerStopDelay.inMinutes
              : null,
          speedStationaryDelay: enableBackground ? 90 : null,
          speedMovingThreshold: enableBackground
              ? const tl.MotionConfig().speedMovingThreshold
              : null,
          // Slow movement must not be overridden by a near-zero GPS reading.
          enableGpsTremorOverride: enableBackground ? false : null,
          speedWakeConfirmCount: enableBackground ? 3 : null,
          stationaryTrackingMode: enableBackground
              ? tl.StationaryTrackingMode.continuous
              : null,
          stationaryPeriodicAccuracy: enableBackground
              ? tl.DesiredAccuracy.low
              : null,
          isMoving: true,
          disableMotionActivityUpdates: !enableMotionActivity,
          motionDetectionMode: useSmartPowerSaving
              ? null
              : tl.MotionDetectionMode.accelerometer,
          disableStopDetection: !useSmartPowerSaving,
        ),
      ),
      persistence: tl.PersistenceConfig(
        // MemoLanes acknowledges the queue after committing each fix.
        maxDaysToPersist: -1,
        persistMode: enableBackground
            ? tl.PersistMode.location
            : tl.PersistMode.none,
        disableProviderChangeRecord: true,
      ),
      http: const tl.HttpConfig(autoSync: false),
    );
  }

  void _onLocation(tl.Location location) {
    _scheduleRecordingDrain();
    try {
      _locations.add(_toLocationData(location));
    } catch (error, stackTrace) {
      log.error(
        '[Tracelet] Location event could not be converted: $error',
        stackTrace,
      );
    }
  }

  static LocationData _toLocationData(tl.Location location) {
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
    return LocationData(
      latitude: coords.latitude,
      longitude: coords.longitude,
      timestampMs: timestampMs,
      accuracy: coords.accuracy,
      altitude: coords.altitude.isFinite ? coords.altitude : null,
      speed: coords.speed.isFinite && coords.speed >= 0 ? coords.speed : null,
    );
  }
}
