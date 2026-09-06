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
          await _m.protect(_recordingCoordinator.tryAutoFinalize);
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
    Fluttertoast.showToast(msg: tr('journey.finalize_saved'));
    if (recordingStatus == GpsRecordingStatus.paused) {
      recordingStatus = GpsRecordingStatus.none;
      notifyListeners();
      await _syncInternalStateWithoutLock();
    }
    _notifyJourneyFinalized();
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
        // The provider stops production and drains its recording consumer
        // before the coordinator is marked inactive.
        await _locationService.stop();
        await _locationUpdateSub?.cancel();
        _locationUpdateSub = null;
        latestPosition = null;
        lastKnownPosition = null;
        _lastPositionTooOldTimer?.cancel();
        _lastPositionTooOldTimer = null;
        if (oldState == _InternalState.recording) {
          await _notificationWhenAppIsKilledPlugin
              .cancelNotificationOnKillService();
          await _recordingCoordinator.stop();
        }
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
    await _initialStateFuture;
    await _providerOperationMutex.protect(() async {
      if (to == GpsRecordingStatus.recording &&
          !await checkAndRequestPermission()) {
        return;
      }

      await _m.protect(() async {
        final needToFinalize =
            recordingStatus != to && to == GpsRecordingStatus.none;
        recordingStatus = to;

        notifyListeners();

        await _syncInternalStateWithoutLock();
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
    if (enable && !await PermissionService().checkLocationPermission()) {
      return false;
    }

    await _providerOperationMutex.protect(() async {
      await _m.protect(() async {
        mapTracking = enable;
        await _syncInternalStateWithoutLock();
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

  void readyToStart() {
    _fullyReady = true;
    unawaited(
      _initialStateFuture.then((_) {
        return _providerOperationMutex.protect(() {
          return _m.protect(() async {
            await _recordingCoordinator.tryAutoFinalize();
            await _syncInternalStateWithoutLock();
          });
        });
      }),
    );
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
