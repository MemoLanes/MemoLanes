import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/widgets.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording/recording_coordinator.dart';
import 'package:memolanes/common/recording_health_service.dart';
import 'package:memolanes/common/service/location/geolocator_service.dart';
import 'package:memolanes/common/service/location/last_known_location.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/common/service/permission_service.dart';
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

class GpsManager extends ChangeNotifier {
  final ILocationService _locationService;
  var recordingStatus = GpsRecordingStatus.none;
  var mapTracking = false;
  LocationData? latestPosition;

  final _journeyFinalizedController = StreamController<void>.broadcast();
  final _recordingDataChangedController = StreamController<void>.broadcast();

  // TODO: In a later version of the achievement system, we should get this
  // notification from the rust side, or pull the backend for updates(the
  // backend query will be every cheap).
  Stream<void> get journeyFinalized => _journeyFinalizedController.stream;

  /// Emitted after a recording batch changes the current journey map.
  Stream<void> get recordingDataChanged =>
      _recordingDataChangedController.stream;

  // OS-cached last known location, used purely as a transient UI fallback
  // while the live stream is still acquiring its first fix. May be arbitrarily
  // stale; never feed this into journey recording. Cleared as soon as a real
  // fix arrives or the location service is turned off.
  LocationData? lastKnownPosition;

  // Keep tracking of the actual internal state which represents the state of
  // gps stream. This is derived from `recordingStatus` and `mapTracking`.
  _InternalState _internalState = _InternalState.off;

  final Mutex _m = Mutex();
  final Mutex _providerOperationMutex = Mutex();
  late final RecordingCoordinator _recordingCoordinator;

  Timer? _lastPositionTooOldTimer;
  Timer? _autoFinalizeTimer;
  bool _disposed = false;

  StreamSubscription<LocationData>? _locationUpdateSub;
  late final Future<void> _initialStateFuture;
  Future<void>? _readyFuture;

  // Notify the user that the recording was unexpectedly stopped.
  // The app is a little hacky so I minted: https://github.com/flutter/flutter/issues/156139
  final _notificationWhenAppIsKilledPlugin = NotificationWhenAppIsKilled();

  // We only start listening to the location service after this.
  // Otherwise we may start it before the app is fully ready (e.g. i18n not ready).
  bool _fullyReady = false;

  GpsManager({LocationServiceFactory? locationServiceFactory})
    : _locationService =
          (locationServiceFactory ?? createDefaultLocationService).call() {
    _recordingCoordinator = RecordingCoordinator(
      onJourneyFinalized: _onJourneyFinalized,
    );
    _initialStateFuture = _initState();
  }

  LocationProviderInfo get locationProvider => _locationService.providerInfo;

  Future<void> _initState() async {
    await _m.protect(() async {
      _autoFinalizeTimer = Timer.periodic(const Duration(minutes: 30), (
        timer,
      ) async {
        await _providerOperationMutex.protect(() async {
          await _m.protect(() async {
            if (_fullyReady) {
              await _recordingCoordinator.tryAutoFinalize();
            }
          });
        });
      });

      if (MMKVUtil.getBool(MMKVKey.isRecording) &&
          await PermissionService().checkLocationPermission()) {
        recordingStatus = GpsRecordingStatus.recording;
      } else if (await api.hasOngoingJourney()) {
        recordingStatus = GpsRecordingStatus.paused;
      }
      // notify record button
      notifyListeners();
    });
  }

  Future<void> _onJourneyFinalized() async {
    await _providerOperationMutex.protect(() async {
      await _m.protect(() async {
        if (_disposed) return;
        Fluttertoast.showToast(msg: tr('journey.finalize_saved'));
        if (recordingStatus == GpsRecordingStatus.paused) {
          recordingStatus = GpsRecordingStatus.none;
          notifyListeners();
          await _syncInternalStateWithoutLock();
          MMKVUtil.putBool(MMKVKey.isRecording, false);
        }
        _notifyJourneyFinalized();
      });
    });
  }

  void _notifyJourneyFinalized() {
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
        await _stopActiveStateWithoutLock(oldState);
      }

      // turnning on if needed
      if (newState != _InternalState.off) {
        log.info("[GpsManager] turning on gps stream. new state: $newState");
        final recording = newState == _InternalState.recording;
        _locationUpdateSub = _locationService.locations.listen(
          _handleDisplayLocation,
        );
        if (recording) await _recordingCoordinator.start();
        try {
          await _locationService.start(
            LocationStartOptions(
              allowBackground: recording,
              recordingConsumer: recording ? _persistLocations : null,
            ),
          );
        } catch (error, stackTrace) {
          log.error(
            '[GpsManager] failed to start location service: $error',
            stackTrace,
          );
          await _locationUpdateSub?.cancel();
          _locationUpdateSub = null;
          if (recording) {
            await _recordingCoordinator.stop();
          }
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
          },
        );

        if (newState == _InternalState.recording) {
          await _configureKillNotificationBestEffort(true);
        }
      }
      _internalState = newState;
      RecordingHealthService.instance.handleRecordingStatus(recordingStatus);
      notifyListeners();
    }
  }

  Future<void> _stopActiveStateWithoutLock(_InternalState oldState) async {
    Object? firstError;
    StackTrace? firstStackTrace;

    Future<void> attempt(Future<void> Function() operation) async {
      try {
        await operation();
      } catch (error, stackTrace) {
        firstError ??= error;
        firstStackTrace ??= stackTrace;
        log.error('[GpsManager] stop cleanup failed: $error', stackTrace);
      }
    }

    // The provider must quiesce its consumer before the coordinator stops.
    // Continue releasing GpsManager-owned resources if provider.stop throws.
    await attempt(_locationService.stop);
    await attempt(() async => _locationUpdateSub?.cancel());
    _locationUpdateSub = null;
    latestPosition = null;
    lastKnownPosition = null;
    _lastPositionTooOldTimer?.cancel();
    _lastPositionTooOldTimer = null;
    if (oldState == _InternalState.recording) {
      await _configureKillNotificationBestEffort(false);
      await attempt(_recordingCoordinator.stop);
    }
    _internalState = _InternalState.off;

    final error = firstError;
    if (error != null) {
      Error.throwWithStackTrace(error, firstStackTrace!);
    }
  }

  Future<void> _configureKillNotificationBestEffort(bool recording) async {
    try {
      if (!recording) {
        await _notificationWhenAppIsKilledPlugin
            .cancelNotificationOnKillService();
        return;
      }
      final enabled =
          await Permission.notification.isGranted &&
          MMKVUtil.getBool(
            MMKVKey.isUnexpectedExitNotificationEnabled,
            defaultValue: true,
          );
      if (!enabled) return;
      await _notificationWhenAppIsKilledPlugin.setNotificationOnKillService(
        ArgsForKillNotification(
          title: tr('unexpected_exit_notification.notification_title'),
          description: tr('unexpected_exit_notification.notification_message'),
          argsForIos: ArgsForIos(
            interruptionLevel: InterruptionLevel.critical,
            useDefaultSound: true,
          ),
        ),
      );
    } catch (error, stackTrace) {
      log.error(
        '[GpsManager] failed to configure kill notification: $error',
        stackTrace,
      );
    }
  }

  void _handleDisplayLocation(LocationData data) {
    if (_disposed || _positionTooOld(data)) return;
    latestPosition = data;
    // First real fix arrived; drop the OS-cached seed so we never silently
    // fall back to a much older position later.
    lastKnownPosition = null;
    notifyListeners();
  }

  Future<void> _persistLocations(
    List<LocationData> locations, {
    required bool isReplay,
  }) async {
    final meaningful = await _recordingCoordinator.persistLocations(
      locations,
      isReplay: isReplay,
    );
    if (meaningful && !_disposed) {
      _recordingDataChangedController.add(null);
    }
  }

  // Non-blocking: fetches the OS-cached last known location and uses it as a
  // transient seed for the map marker while the live stream warms up. Has no
  // effect once a real fix has already arrived or the service has stopped.
  Future<void> _seedLastKnownPosition() async {
    final seed = await getLastKnownLocation();
    if (seed == null) return;
    if (_disposed) return;
    if (latestPosition != null) return;
    if (_internalState == _InternalState.off) return;
    lastKnownPosition = seed;
    notifyListeners();
  }

  Future<void> changeRecordingState(GpsRecordingStatus to) async {
    await readyToStart();
    await _providerOperationMutex.protect(() async {
      if (to == GpsRecordingStatus.recording &&
          !await checkAndRequestPermission()) {
        return;
      }

      await _m.protect(() async {
        final previousStatus = recordingStatus;
        final needToFinalize =
            previousStatus != to && to == GpsRecordingStatus.none;
        recordingStatus = to;

        notifyListeners();

        try {
          await _syncInternalStateWithoutLock();
        } catch (error, stackTrace) {
          recordingStatus = previousStatus;
          notifyListeners();
          try {
            await _syncInternalStateWithoutLock();
          } catch (restoreError, restoreStackTrace) {
            log.error(
              '[GpsManager] failed to restore previous state: $restoreError',
              restoreStackTrace,
            );
          }
          Error.throwWithStackTrace(error, stackTrace);
        }
        MMKVUtil.putBool(
          MMKVKey.isRecording,
          recordingStatus == GpsRecordingStatus.recording,
        );

        if (needToFinalize) {
          if (await api.finalizeOngoingJourney()) {
            Fluttertoast.showToast(msg: tr('journey.finalize_saved'));
          } else {
            Fluttertoast.showToast(msg: tr('journey.finalize_empty'));
          }
          _notifyJourneyFinalized();
        }
      });
    });
  }

  Future<bool> toggleMapTracking(bool enable) async {
    await readyToStart();
    if (enable && !await PermissionService().checkLocationPermission()) {
      return false;
    }

    await _providerOperationMutex.protect(() async {
      await _m.protect(() async {
        final previousMapTracking = mapTracking;
        mapTracking = enable;
        try {
          await _syncInternalStateWithoutLock();
        } catch (error, stackTrace) {
          mapTracking = previousMapTracking;
          notifyListeners();
          try {
            await _syncInternalStateWithoutLock();
          } catch (restoreError, restoreStackTrace) {
            log.error(
              '[GpsManager] failed to restore map tracking: $restoreError',
              restoreStackTrace,
            );
          }
          Error.throwWithStackTrace(error, stackTrace);
        }
      });
    });
    return true;
  }

  Future<void> handleAppForeground() => _providerOperationMutex.protect(
    () => _locationService.setForeground(true),
  );

  Future<void> handleAppBackground() => _providerOperationMutex.protect(
    () => _locationService.setForeground(false),
  );

  Future<void> readyToStart() {
    final existing = _readyFuture;
    if (existing != null) return existing;
    final attempt = _readyToStart();
    _readyFuture = attempt;
    return attempt;
  }

  Future<void> _readyToStart() async {
    await _initialStateFuture;
    try {
      await _providerOperationMutex.protect(() async {
        await _m.protect(() async {
          try {
            final meaningful = await _recordingCoordinator.recoverPending(
              _locationService.recoverPendingDeliveries,
              remainActive: recordingStatus == GpsRecordingStatus.recording,
            );
            if (meaningful && !_disposed) {
              _recordingDataChangedController.add(null);
            }
            _fullyReady = true;
            await _syncInternalStateWithoutLock();
          } catch (_) {
            // Settle startup state before releasing the provider mutex. An
            // auto-finalize callback queued during recovery will then observe
            // paused and can complete the paused-to-none transition.
            _fullyReady = false;
            if (recordingStatus == GpsRecordingStatus.recording) {
              recordingStatus = GpsRecordingStatus.paused;
              MMKVUtil.putBool(MMKVKey.isRecording, false);
              RecordingHealthService.instance.handleRecordingStatus(
                recordingStatus,
              );
              notifyListeners();
            }
            rethrow;
          }
        });
      });
    } catch (error, stackTrace) {
      _readyFuture = null;
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _lastPositionTooOldTimer?.cancel();
    _autoFinalizeTimer?.cancel();
    unawaited(_disposeAsync());
    RecordingHealthService.instance.stop();
    super.dispose();
  }

  Future<void> _disposeAsync() async {
    try {
      await _providerOperationMutex.protect(() async {
        await _locationService.dispose();
        await _locationUpdateSub?.cancel();
        await _recordingCoordinator.dispose();
      });
    } catch (error, stackTrace) {
      log.error('[GpsManager] dispose failed: $error', stackTrace);
    } finally {
      await _journeyFinalizedController.close();
      await _recordingDataChangedController.close();
    }
  }
}
