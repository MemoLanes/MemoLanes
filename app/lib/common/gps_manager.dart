import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/widgets.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/journey_idle_timer.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording_health_service.dart';
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

class GpsManager extends ChangeNotifier {
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

  StreamSubscription<LocationData>? _locationUpdateSub;

  final _recordingQueue = Mutex();

  // Notify the user that the recording was unexpectedly stopped.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

  late final _idleTimer = JourneyIdleTimer(_checkIdleJourney);
  Timer? _pausedResumeTimer;
  late final AppLifecycleListener _lifecycle;

  // We only start listening to the location service after this.
  // Otherwise we may start it before the app is fully ready (e.g. i18n not ready).
  bool _fullyReady = false;

  GpsManager({bool? wasRecording}) {
    _locationService = GeoLocatorService();
    _lifecycle = AppLifecycleListener(onResume: _onResume);
    _initState(wasRecording ?? MMKVUtil.getBool(MMKVKey.isRecording));
  }

  LocationBackend get locationBackend => _locationService.locationBackend;

  void _initState(bool wasRecording) async {
    await _m.protect(() async {
      if (wasRecording && await PermissionService().checkLocationPermission()) {
        recordingStatus = GpsRecordingStatus.recording;
      } else if (await api.hasOngoingJourney()) {
        recordingStatus = GpsRecordingStatus.paused;
      }
      // notify record button
      notifyListeners();
    });
  }

  void _onResume() {
    if (!_fullyReady) return;
    if (recordingStatus == GpsRecordingStatus.recording) {
      _idleTimer.onResume();
    } else if (recordingStatus == GpsRecordingStatus.paused) {
      // Let foreground work settle before checking a paused journey.
      _pausedResumeTimer ??= Timer(const Duration(seconds: 5), () {
        _pausedResumeTimer = null;
        unawaited(
          _checkIdleJourney(() => recordingStatus == GpsRecordingStatus.paused),
        );
      });
    }
  }

  Future<void> _checkIdleJourney(bool Function() isCurrent) async {
    try {
      await _m.protect(() async {
        if (recordingStatus == GpsRecordingStatus.none || !isCurrent()) return;
        _locationService.flushPendingUpdates();
        final finalized = await _recordingQueue.protect(
          () async => isCurrent() && await api.tryAutoFinalizeJourney(),
        );
        if (finalized) await _handleIdleFinalization();
      });
    } catch (error, stackTrace) {
      log.error("[GpsManager] idle finalization failed: $error", stackTrace);
    }
  }

  Future<void> _handleIdleFinalization() async {
    if (recordingStatus == GpsRecordingStatus.paused) {
      recordingStatus = GpsRecordingStatus.none;
      notifyListeners();
      await _syncInternalStateWithoutLock();
    }
    _showJourneySaved();
  }

  void _showJourneySaved() {
    Fluttertoast.showToast(msg: tr("journey.finalize_saved"));
    _journeyFinalizedController.add(null);
  }

  Future<void> _syncInternalStateWithoutLock() async {
    // do nothing until fully ready, we will sync it again when it becomes ready
    // for the first time.
    if (!_fullyReady) {
      return;
    }

    var newState = switch (recordingStatus) {
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
    var oldState = _internalState;
    if (oldState != newState) {
      // state changed

      // turning off if needed
      if (oldState != _InternalState.off) {
        await _locationService.stopLocationUpdates();
        await _locationUpdateSub?.cancel();
        _locationUpdateSub = null;
        latestPosition = null;
        lastKnownPosition = null;
        _lastPositionTooOldTimer?.cancel();
        _lastPositionTooOldTimer = null;
        if (oldState == _InternalState.recording) {
          // The stopped source has flushed its last batch into the queue.
          await _recordingQueue.protect(() async {});
          await _notificationWhenAppIsKilledPlugin
              .cancelNotificationOnKillService();
        }
      }

      // turnning on if needed
      if (newState != _InternalState.off) {
        log.info("[GpsManager] turning on gps stream. new state: $newState");
        bool enableBackground = newState == _InternalState.recording;
        // A backend can deliver its first fix before startLocationUpdates
        // completes. Subscribe first so a broadcast stream cannot drop it.
        _internalState = newState;
        final recordingStartedAtMs = MMKVUtil.getInt(
          MMKVKey.recordingStartedAtMs,
        );
        _locationUpdateSub = _locationService.onLocationUpdate((data) {
          if (_internalState == _InternalState.recording &&
              data.timestampMs >= recordingStartedAtMs) {
            final receivedAt = DateTime.now().millisecondsSinceEpoch;
            unawaited(_recordingQueue.protect(() => _record(data, receivedAt)));
          }
          if (_positionTooOld(data)) {
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
          await _locationUpdateSub?.cancel();
          _locationUpdateSub = null;
          _internalState = _InternalState.off;
          latestPosition = null;
          lastKnownPosition = null;
          notifyListeners();
          log.error(
            "[GpsManager] failed to start gps stream: $error",
            stackTrace,
          );
          rethrow;
        }
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
      _internalState = newState;
      RecordingHealthService.instance.handleRecordingStatus(recordingStatus);
      notifyListeners();
    }
  }

  // Non-blocking: fetches the OS-cached last known location and uses it as a
  // transient seed for the map marker while the live stream warms up. Has no
  // effect once a real fix has already arrived or the service has stopped.
  Future<void> _seedLastKnownPosition() async {
    final seed = await getLastKnownLocation();
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

  Future<void> _record(LocationData data, int receivedAtMs) async {
    try {
      final result = await api.onLocationUpdate(
        data: ExtendedRawGPSPoint(
          rawGpsPoint: RawGPSPoint(
            point: Point(latitude: data.latitude, longitude: data.longitude),
            timestampMs: data.timestampMs,
            accuracy: data.accuracy,
            altitude: data.altitude,
            speed: data.speed,
          ),
          receivedTimestampMs: receivedAtMs,
        ),
      );
      if (result.previousJourneyFinalized) _showJourneySaved();
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
      final needToFinalize =
          recordingStatus != to && to == GpsRecordingStatus.none;
      if (to == GpsRecordingStatus.recording && recordingStatus != to) {
        MMKVUtil.putInt(
          MMKVKey.recordingStartedAtMs,
          DateTime.now().millisecondsSinceEpoch,
        );
      }
      recordingStatus = to;

      notifyListeners();

      await _syncInternalStateWithoutLock();
      MMKVUtil.putBool(
        MMKVKey.isRecording,
        recordingStatus == GpsRecordingStatus.recording,
      );

      if (needToFinalize) {
        if (await api.finalizeOngoingJourney()) {
          Fluttertoast.showToast(msg: tr("journey.finalize_saved"));
        } else {
          Fluttertoast.showToast(msg: tr("journey.finalize_empty"));
        }
        _journeyFinalizedController.add(null);
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
    _idleTimer.restart();
    // sync internal state for the first time
    unawaited(
      _m
          .protect(() async {
            _fullyReady = true;
            // A restored recording may still receive fixes sampled before launch.
            if (recordingStatus == GpsRecordingStatus.paused &&
                await api.tryAutoFinalizeJourney()) {
              await _handleIdleFinalization();
            }
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
    _lifecycle.dispose();
    _idleTimer.cancel();
    _pausedResumeTimer?.cancel();
    _lastPositionTooOldTimer?.cancel();
    _locationService.flushPendingUpdates();
    unawaited(_locationUpdateSub?.cancel());
    unawaited(_locationService.stopLocationUpdates());
    unawaited(_recordingQueue.protect(_journeyFinalizedController.close));
    RecordingHealthService.instance.stop();
    super.dispose();
  }
}
