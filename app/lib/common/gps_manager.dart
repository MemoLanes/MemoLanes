import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/widgets.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording_health_service.dart';
import 'package:memolanes/common/recording_queue.dart';
import 'package:memolanes/common/service/location/geolocator_service.dart';
import 'package:memolanes/common/service/location/last_known_location.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:memolanes/src/rust/gps.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:mutex/mutex.dart';
import 'package:notification_when_app_is_killed/model/args_for_ios.dart';
import 'package:notification_when_app_is_killed/model/args_for_kill_notification.dart';
import 'package:notification_when_app_is_killed/notification_when_app_is_killed.dart';
import 'package:permission_handler/permission_handler.dart';

enum GpsRecordingStatus { none, recording, paused }

// `recording` requires background location but `justForTracking` does not.
enum _InternalState { off, recording, justForTracking }

bool _positionTooOld(LocationData data, {int staleThresholdMs = 12 * 1000}) {
  final now = DateTime.now().millisecondsSinceEpoch;
  return now - data.timestampMs >= staleThresholdMs;
}

const _lastKnownPositionMaxAge = Duration(seconds: 30);

class GpsManager extends ChangeNotifier with WidgetsBindingObserver {
  late final ILocationService _locationService;
  var recordingStatus = GpsRecordingStatus.none;
  var mapTracking = false;
  LocationData? latestPosition;

  final _journeyFinalizedController = StreamController<void>.broadcast();

  // TODO: In a later version of the achievement system, we should get this
  // notification from the rust side, or pull the backend for updates(the
  // backend query will be every cheap).
  Stream<void> get journeyFinalized => _journeyFinalizedController.stream;

  // OS-cached last known location, used purely as a transient UI fallback
  // while the live stream is still acquiring its first fix. Never feed this
  // into journey recording. Cleared when it expires, a real fix arrives, or
  // the location service is turned off.
  LocationData? lastKnownPosition;

  // Keep tracking of the actual internal state which represents the state of
  // gps stream. This is derived from `recordingStatus` and `mapTracking`.
  _InternalState _internalState = _InternalState.off;

  final Mutex _m = Mutex();

  Timer? _lastPositionTooOldTimer;
  Timer? _pausedFinalizeTimer;
  bool _disposed = false;

  StreamSubscription<LocationData>? _locationUpdateSub;

  late final RecordingQueue _recordingQueue = RecordingQueue(
    onJourneyFinalized: _notifyJourneyFinalized,
  );

  // Notify the user that the recording was unexpectedly stopped.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

  // We only start listening to the location service after this.
  // Otherwise we may start it before the app is fully ready (e.g. i18n not ready).
  bool _fullyReady = false;

  GpsManager() {
    _locationService = GeoLocatorService();
    unawaited(_initState());
  }

  LocationBackend get locationBackend => _locationService.locationBackend;

  Future<void> _initState() async {
    try {
      await _m.protect(() async {
        if (_disposed) return;
        if (MMKVUtil.getBool(MMKVKey.isRecording) &&
            await PermissionService().checkLocationPermission()) {
          recordingStatus = GpsRecordingStatus.recording;
        } else if (await api.hasOngoingJourney()) {
          recordingStatus = GpsRecordingStatus.paused;
        }
        if (!_disposed) notifyListeners();
      });
    } catch (error, stackTrace) {
      log.error(
        '[GpsManager] restore recording status failed: $error',
        stackTrace,
      );
    }
  }

  Future<void> _tryFinalizePausedWithoutLock() async {
    if (_fullyReady &&
        !_disposed &&
        recordingStatus == GpsRecordingStatus.paused &&
        _internalState != _InternalState.recording &&
        !_recordingQueue.hasPending &&
        await api.tryFinalizePausedJourney()) {
      recordingStatus = GpsRecordingStatus.none;
      _notifyJourneyFinalized();
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> _checkPausedJourney() async {
    try {
      await _m.protect(_tryFinalizePausedWithoutLock);
    } catch (error, stackTrace) {
      log.error('[GpsManager] paused journey check failed: $error', stackTrace);
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(_checkPausedJourney());
    }
  }

  void _notifyJourneyFinalized({bool saved = true}) {
    if (_disposed) return;
    Fluttertoast.showToast(
      msg: saved ? tr("journey.finalize_saved") : tr("journey.finalize_empty"),
    );
    _journeyFinalizedController.add(null);
  }

  Future<void> _stopAcquisitionWithoutLock() async {
    final wasRecording = _internalState == _InternalState.recording;
    await _locationService.stopLocationUpdates();
    await _locationUpdateSub?.cancel();
    _locationUpdateSub = null;
    latestPosition = null;
    lastKnownPosition = null;
    _lastPositionTooOldTimer?.cancel();
    _lastPositionTooOldTimer = null;
    _internalState = _InternalState.off;
    if (wasRecording) {
      try {
        await _notificationWhenAppIsKilledPlugin
            .cancelNotificationOnKillService();
      } catch (error, stackTrace) {
        log.error(
          '[GpsManager] cancel unexpected exit notification failed: $error',
          stackTrace,
        );
      }
    }
  }

  Future<void> _syncInternalStateWithoutLock([
    GpsRecordingStatus? requestedStatus,
  ]) async {
    // do nothing until fully ready, we will sync it again when it becomes ready
    // for the first time.
    if (!_fullyReady || _disposed) {
      return;
    }

    final targetStatus = requestedStatus ?? recordingStatus;
    var newState = switch (targetStatus) {
      GpsRecordingStatus.recording => _InternalState.recording,
      GpsRecordingStatus.paused ||
      GpsRecordingStatus.none => switch (mapTracking) {
        true => _InternalState.justForTracking,
        false => _InternalState.off,
      },
    };
    final lifecycleStateAtSync = WidgetsBinding.instance.lifecycleState;
    final allowTrackingOnlyGps =
        lifecycleStateAtSync == null ||
        lifecycleStateAtSync == AppLifecycleState.resumed;
    if (!allowTrackingOnlyGps && newState == _InternalState.justForTracking) {
      // Map tracking should only use GPS when app is visible.
      newState = _InternalState.off;
    }
    final oldState = _internalState;
    if (oldState != newState) {
      if (oldState != _InternalState.off) {
        await _stopAcquisitionWithoutLock();
      }
      if (_disposed) return;

      // turnning on if needed
      if (newState != _InternalState.off) {
        log.info("[GpsManager] turning on gps stream. new state: $newState");
        final enableBackground = newState == _InternalState.recording;
        // A backend can deliver its first fix before startLocationUpdates
        // completes. Subscribe first so a broadcast stream cannot drop it.
        _internalState = newState;
        _locationUpdateSub = _locationService.onLocationUpdate((data) {
          if (enableBackground) unawaited(_recordLocationUpdate(data));
          if (_disposed || _positionTooOld(data)) {
            return;
          }
          latestPosition = data;
          // First real fix arrived; drop the OS-cached seed so we never
          // silently fall back to a much older position later.
          if (lastKnownPosition != null) {
            lastKnownPosition = null;
          }
          notifyListeners();
        });
        try {
          await _locationService.startLocationUpdates(enableBackground);
        } catch (error, stackTrace) {
          log.error(
            "[GpsManager] failed to start gps stream: $error",
            stackTrace,
          );
          await _stopAcquisitionWithoutLock();
          rethrow;
        }
        if (_disposed) return;
        unawaited(_seedLastKnownPosition());

        _lastPositionTooOldTimer ??= Timer.periodic(
          const Duration(seconds: 1),
          (timer) {
            var latestPosition = this.latestPosition;
            if (latestPosition != null) {
              if (_positionTooOld(latestPosition)) {
                this.latestPosition = null;
                notifyListeners();
              }
            }
            final lastKnownPosition = this.lastKnownPosition;
            if (lastKnownPosition != null &&
                !_lastKnownPositionIsFresh(lastKnownPosition)) {
              this.lastKnownPosition = null;
              notifyListeners();
            }
          },
        );

        final unexpectedExitNotificationStatus =
            await Permission.notification.isGranted &&
            MMKVUtil.getBool(
              MMKVKey.isUnexpectedExitNotificationEnabled,
              defaultValue: true,
            );
        if (newState == _InternalState.recording &&
            unexpectedExitNotificationStatus) {
          await _notificationWhenAppIsKilledPlugin.setNotificationOnKillService(
            ArgsForKillNotification(
              title: tr("unexpected_exit_notification.notification_title"),
              description: tr(
                "unexpected_exit_notification.notification_message",
              ),
              argsForIos: ArgsForIos(
                interruptionLevel: InterruptionLevel.critical,
                useDefaultSound: true,
              ),
            ),
          );
        }
      }
      if (!_disposed) {
        RecordingHealthService.instance.handleRecordingStatus(targetStatus);
        if (requestedStatus == null) notifyListeners();
      }
    }
  }

  // Non-blocking: fetches the OS-cached last known location and uses it as a
  // transient seed for the map marker while the live stream warms up. Has no
  // effect once a real fix has already arrived or the service has stopped.
  Future<void> _seedLastKnownPosition() async {
    final seed = await getLastKnownLocation();
    if (_disposed) return;
    if (seed == null || !_lastKnownPositionIsFresh(seed)) return;
    if (latestPosition != null) return;
    if (_internalState == _InternalState.off) return;
    lastKnownPosition = seed;
    notifyListeners();
  }

  bool _lastKnownPositionIsFresh(LocationData data) {
    final ageMs = DateTime.now().millisecondsSinceEpoch - data.timestampMs;
    return ageMs >= 0 && ageMs <= _lastKnownPositionMaxAge.inMilliseconds;
  }

  Future<void> _recordLocationUpdate(LocationData data) async {
    try {
      await _recordingQueue.add(
        ExtendedRawGPSPoint(
          rawGpsPoint: RawGPSPoint(
            point: Point(latitude: data.latitude, longitude: data.longitude),
            timestampMs: data.timestampMs,
            accuracy: data.accuracy,
            altitude: data.altitude,
            speed: data.speed,
          ),
          receivedTimestampMs: data.receivedTimestampMs,
        ),
      );
    } catch (error, stackTrace) {
      log.error(
        "[GpsManager] record location update failed: $error",
        stackTrace,
      );
    }
  }

  Future<void> changeRecordingState(GpsRecordingStatus to) async {
    if (to == GpsRecordingStatus.recording) {
      if (!await checkAndRequestPermission()) {
        return;
      }
    }

    await _m.protect(() async {
      if (_disposed) return;
      final needToFinalize =
          recordingStatus != GpsRecordingStatus.none &&
          to == GpsRecordingStatus.none;
      try {
        if (to == GpsRecordingStatus.recording &&
            recordingStatus == GpsRecordingStatus.paused) {
          await _recordingQueue.drain();
          await _tryFinalizePausedWithoutLock();
        }
        await _syncInternalStateWithoutLock(to);
        if (to != GpsRecordingStatus.recording) await _recordingQueue.drain();
        if (_disposed && to == GpsRecordingStatus.recording) return;
        if (needToFinalize) {
          _notifyJourneyFinalized(saved: await api.finalizeOngoingJourney());
        }
        recordingStatus = to;
      } catch (_) {
        if (_internalState == _InternalState.recording) {
          recordingStatus = GpsRecordingStatus.recording;
        } else if (recordingStatus != GpsRecordingStatus.none ||
            _recordingQueue.hasPending) {
          recordingStatus = GpsRecordingStatus.paused;
        }
        if (!_disposed) {
          RecordingHealthService.instance.handleRecordingStatus(
            recordingStatus,
          );
        }
        rethrow;
      } finally {
        MMKVUtil.putBool(
          MMKVKey.isRecording,
          recordingStatus == GpsRecordingStatus.recording,
        );
        if (!_disposed) notifyListeners();
      }
    });
  }

  Future<bool> toggleMapTracking(bool enable) async {
    if (enable && !await PermissionService().checkLocationPermission()) {
      return false;
    }

    await _m.protect(() async {
      mapTracking = enable;
      await _syncInternalStateWithoutLock();
    });
    return true;
  }

  void readyToStart() {
    if (_fullyReady || _disposed) return;
    _fullyReady = true;
    WidgetsBinding.instance.addObserver(this);
    _pausedFinalizeTimer = Timer.periodic(
      const Duration(minutes: 30),
      (_) => unawaited(_checkPausedJourney()),
    );
    unawaited(
      _m
          .protect(() async {
            await _tryFinalizePausedWithoutLock();
            await _syncInternalStateWithoutLock();
          })
          .catchError((Object error, StackTrace stackTrace) {
            log.error(
              '[GpsManager] initial state sync failed: $error',
              stackTrace,
            );
          }),
    );
  }

  @override
  void dispose() {
    _disposed = true;
    WidgetsBinding.instance.removeObserver(this);
    _lastPositionTooOldTimer?.cancel();
    _pausedFinalizeTimer?.cancel();
    unawaited(
      _m
          .protect(() async {
            await _stopAcquisitionWithoutLock();
            await _recordingQueue.drain();
          })
          .catchError((Object error, StackTrace stackTrace) {
            log.error(
              '[GpsManager] stop recording on dispose failed: $error',
              stackTrace,
            );
          }),
    );
    unawaited(_journeyFinalizedController.close());
    RecordingHealthService.instance.stop();
    super.dispose();
  }
}
