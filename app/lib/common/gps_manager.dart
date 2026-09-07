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
  final Future<GpsRecordingStatus> Function()? _initialStateLoader;
  final Future<bool> Function()? _permissionRequester;
  final Future<bool> Function()? _finalizeJourneyOverride;
  final Future<void> Function(bool)? _persistRecordingStateOverride;
  final Future<void> Function(bool)? _configureKillNotificationOverride;
  final void Function(String)? _showToastOverride;
  final void Function(GpsRecordingStatus)? _recordingHealthUpdaterOverride;
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

  GpsManager({
    LocationServiceFactory? locationServiceFactory,
    Future<GpsRecordingStatus> Function()? initialStateLoader,
    Future<bool> Function()? permissionRequester,
    Future<bool> Function()? finalizeJourney,
    Future<void> Function(bool)? persistRecordingState,
    Future<void> Function(bool)? configureKillNotification,
    void Function(String)? showToast,
    void Function(GpsRecordingStatus)? recordingHealthUpdater,
    Future<bool> Function(List<RecordingLocationUpdate> updates)?
    recordingWriter,
    Future<bool> Function()? tryAutoFinalize,
  }) : // Publicly named test seam initializes a private implementation field.
       // ignore: prefer_initializing_formals
       _initialStateLoader = initialStateLoader,
       // ignore: prefer_initializing_formals
       _permissionRequester = permissionRequester,
       _finalizeJourneyOverride = finalizeJourney,
       _persistRecordingStateOverride = persistRecordingState,
       _configureKillNotificationOverride = configureKillNotification,
       _showToastOverride = showToast,
       _recordingHealthUpdaterOverride = recordingHealthUpdater,
       _locationService =
           (locationServiceFactory ?? createDefaultLocationService).call() {
    _recordingCoordinator = RecordingCoordinator(
      onJourneyFinalized: _onJourneyFinalized,
      onLocationUpdates: recordingWriter,
      tryAutoFinalize: tryAutoFinalize,
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

      final initialStateLoader = _initialStateLoader;
      if (initialStateLoader != null) {
        recordingStatus = await initialStateLoader();
      } else if (MMKVUtil.getBool(MMKVKey.isRecording) &&
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
    // A provider may be awaiting its recording consumer while finalization is
    // running. Never synchronously re-enter provider lifecycle operations from
    // that delivery stack.
    scheduleMicrotask(() => unawaited(_applyFinalizedJourneyState()));
  }

  Future<void> _applyFinalizedJourneyState() async {
    try {
      await _providerOperationMutex.protect(() async {
        await _m.protect(() async {
          if (_disposed) return;
          if (recordingStatus == GpsRecordingStatus.paused) {
            await _commitStateWithoutLock(
              desiredRecordingStatus: GpsRecordingStatus.none,
              desiredMapTracking: mapTracking,
            );
          }
        });
      });
      if (_disposed) return;
      _showToast(tr('journey.finalize_saved'));
      _notifyJourneyFinalized();
    } catch (error, stackTrace) {
      log.error(
        '[GpsManager] failed to apply finalized journey state: $error',
        stackTrace,
      );
    }
  }

  void _notifyJourneyFinalized() {
    _journeyFinalizedController.add(null);
  }

  _InternalState _desiredInternalState(
    GpsRecordingStatus desiredRecordingStatus,
    bool desiredMapTracking,
  ) {
    if (!_fullyReady) return _InternalState.off;
    var newState = switch (desiredRecordingStatus) {
      GpsRecordingStatus.recording => _InternalState.recording,
      GpsRecordingStatus.paused ||
      GpsRecordingStatus.none => switch (desiredMapTracking) {
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
    return newState;
  }

  Future<void> _transitionInternalStateWithoutLock(
    _InternalState newState,
  ) async {
    final oldState = _internalState;
    if (oldState == newState) return;

    if (oldState != _InternalState.off) {
      await _stopActiveState(oldState);
    }
    if (newState != _InternalState.off) {
      await _startActiveState(newState);
    }
  }

  Future<void> _stopActiveState(_InternalState oldState) async {
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

    // Stop production and wait for provider-owned deliveries before stopping
    // the consumer. Continue cleanup even when one platform operation fails.
    await attempt(_locationService.stop);
    await attempt(() async => _locationUpdateSub?.cancel());
    _locationUpdateSub = null;
    latestPosition = null;
    lastKnownPosition = null;
    _lastPositionTooOldTimer?.cancel();
    _lastPositionTooOldTimer = null;
    if (oldState == _InternalState.recording) {
      await attempt(() => _configureKillNotification(false));
      await attempt(_recordingCoordinator.stop);
    }
    _internalState = _InternalState.off;

    final error = firstError;
    if (error != null) {
      Error.throwWithStackTrace(error, firstStackTrace!);
    }
  }

  Future<void> _startActiveState(_InternalState newState) async {
    log.info('[GpsManager] turning on gps stream. new state: $newState');
    final recording = newState == _InternalState.recording;
    try {
      _locationUpdateSub = _locationService.locations.listen(
        _handleDisplayLocation,
      );
      if (recording) await _recordingCoordinator.start();
      await _locationService.start(
        LocationStartOptions(
          allowBackground: recording,
          recordingConsumer: recording ? _persistLocations : null,
        ),
      );
      if (recording) await _configureKillNotification(true);

      _lastPositionTooOldTimer ??= Timer.periodic(const Duration(seconds: 1), (
        timer,
      ) {
        final position = latestPosition;
        if (position != null && _positionTooOld(position)) {
          latestPosition = null;
          notifyListeners();
        }
      });
      _internalState = newState;
      unawaited(_seedLastKnownPosition());
    } catch (error, stackTrace) {
      log.error(
        '[GpsManager] failed to start location service: $error',
        stackTrace,
      );
      await _cleanupPartialStart(recording: recording);
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  Future<void> _cleanupPartialStart({required bool recording}) async {
    Future<void> attempt(Future<void> Function() operation) async {
      try {
        await operation();
      } catch (error, stackTrace) {
        log.error(
          '[GpsManager] partial-start cleanup failed: $error',
          stackTrace,
        );
      }
    }

    await attempt(_locationService.stop);
    await attempt(() async => _locationUpdateSub?.cancel());
    _locationUpdateSub = null;
    _lastPositionTooOldTimer?.cancel();
    _lastPositionTooOldTimer = null;
    latestPosition = null;
    lastKnownPosition = null;
    if (recording) {
      await attempt(() => _configureKillNotification(false));
      await attempt(_recordingCoordinator.stop);
    }
    _internalState = _InternalState.off;
  }

  Future<void> _commitStateWithoutLock({
    required GpsRecordingStatus desiredRecordingStatus,
    required bool desiredMapTracking,
  }) async {
    final desiredInternalState = _desiredInternalState(
      desiredRecordingStatus,
      desiredMapTracking,
    );
    try {
      await _transitionInternalStateWithoutLock(desiredInternalState);
      await _persistRecordingState(
        desiredRecordingStatus == GpsRecordingStatus.recording,
      );
    } catch (error, stackTrace) {
      await _forceSafeOffWithoutLock();
      Error.throwWithStackTrace(error, stackTrace);
    }

    recordingStatus = desiredRecordingStatus;
    mapTracking = desiredMapTracking;
    _updateRecordingHealth();
    notifyListeners();
  }

  Future<void> _forceSafeOffWithoutLock() async {
    try {
      if (_internalState != _InternalState.off) {
        await _stopActiveState(_internalState);
      } else {
        await _cleanupPartialStart(
          recording: recordingStatus == GpsRecordingStatus.recording,
        );
      }
    } catch (error, stackTrace) {
      log.error('[GpsManager] safe-off cleanup failed: $error', stackTrace);
    }
    _internalState = _InternalState.off;
    recordingStatus = GpsRecordingStatus.none;
    mapTracking = false;
    try {
      await _persistRecordingState(false);
    } catch (error, stackTrace) {
      log.error('[GpsManager] failed to persist safe-off: $error', stackTrace);
    }
    _updateRecordingHealth();
    notifyListeners();
  }

  Future<void> _persistRecordingState(bool recording) async {
    final persistRecordingState = _persistRecordingStateOverride;
    if (persistRecordingState != null) {
      await persistRecordingState(recording);
      return;
    }
    if (!MMKVUtil.putBool(MMKVKey.isRecording, recording)) {
      throw StateError('Failed to persist recording state');
    }
  }

  Future<void> _configureKillNotification(bool recording) async {
    final configure = _configureKillNotificationOverride;
    if (configure != null) {
      await configure(recording);
      return;
    }
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
  }

  void _showToast(String message) {
    final showToast = _showToastOverride;
    if (showToast != null) {
      showToast(message);
    } else {
      Fluttertoast.showToast(msg: message);
    }
  }

  void _updateRecordingHealth() {
    final updater = _recordingHealthUpdaterOverride;
    if (updater != null) {
      updater(recordingStatus);
    } else {
      RecordingHealthService.instance.handleRecordingStatus(recordingStatus);
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
          !await (_permissionRequester?.call() ??
              checkAndRequestPermission())) {
        return;
      }

      await _m.protect(() async {
        final needToFinalize =
            recordingStatus != to && to == GpsRecordingStatus.none;
        await _commitStateWithoutLock(
          desiredRecordingStatus: to,
          desiredMapTracking: mapTracking,
        );

        if (needToFinalize) {
          final finalized =
              await (_finalizeJourneyOverride?.call() ??
                  api.finalizeOngoingJourney());
          if (finalized) {
            _showToast(tr('journey.finalize_saved'));
          } else {
            _showToast(tr('journey.finalize_empty'));
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
        await _commitStateWithoutLock(
          desiredRecordingStatus: recordingStatus,
          desiredMapTracking: enable,
        );
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
    return _readyFuture ??= _readyToStart();
  }

  Future<void> _readyToStart() async {
    await _initialStateFuture;
    try {
      await _providerOperationMutex.protect(() async {
        await _m.protect(() async {
          _fullyReady = true;

          // Recovery must complete before auto-finalizing an old journey. The
          // coordinator is started temporarily even when the restored public
          // state is paused or off so recovered data has a valid consumer.
          await _recordingCoordinator.start();
          await _locationService.recoverPendingDeliveries(_persistLocations);
          await _recordingCoordinator.tryAutoFinalize();
          if (recordingStatus != GpsRecordingStatus.recording) {
            await _recordingCoordinator.stop();
          }
          await _commitStateWithoutLock(
            desiredRecordingStatus: recordingStatus,
            desiredMapTracking: mapTracking,
          );
        });
      });
    } catch (error, stackTrace) {
      log.error('[GpsManager] startup recovery failed: $error', stackTrace);
      await _providerOperationMutex.protect(() async {
        await _m.protect(_forceSafeOffWithoutLock);
      });
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
