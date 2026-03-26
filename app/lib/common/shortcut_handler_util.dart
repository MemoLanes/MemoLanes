import 'dart:async';

import 'package:flutter_app_intents/flutter_app_intents.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';

class ShortcutHandlerUtil {
  ShortcutHandlerUtil._();

  static GpsManager? _gps;

  static void init({required GpsManager gpsManager}) {
    _gps = gpsManager;
    unawaited(_registerIntents());
  }

  static Future<void> _registerIntents() async {
    final client = FlutterAppIntentsClient.instance;

    await _register(
      client,
      id: 'com.memolanes.StartRecordingIntent',
      title: 'Start Recording',
      description: 'Start recording a journey in MemoLanes',
      status: GpsRecordingStatus.recording,
      okValue: 'Started recording',
    );
    await _register(
      client,
      id: 'com.memolanes.StopRecordingIntent',
      title: 'Stop Recording',
      description: 'Stop recording and save the journey in MemoLanes',
      status: GpsRecordingStatus.none,
      okValue: 'Stopped recording',
    );
    await _register(
      client,
      id: 'com.memolanes.PauseRecordingIntent',
      title: 'Pause Recording',
      description: 'Pause the current journey recording in MemoLanes',
      status: GpsRecordingStatus.paused,
      okValue: 'Paused recording',
    );

    await client.updateShortcuts();
  }

  static Future<void> _register(
    FlutterAppIntentsClient client, {
    required String id,
    required String title,
    required String description,
    required GpsRecordingStatus status,
    required String okValue,
  }) async {
    final intent = AppIntentBuilder()
        .identifier(id)
        .title(title)
        .description(description)
        .build();

    await client.registerIntent(intent, (_) async {
      final gps = _gps;
      if (gps == null) {
        final err = 'GpsManager not bound';
        log.warning('$id: $err');
        return AppIntentResult.failed(error: err);
      }
      try {
        await gps.changeRecordingState(status);
        return AppIntentResult.successful(value: okValue);
      } catch (e, s) {
        log.error('$id failed: $e', s);
        return AppIntentResult.failed(error: '$e');
      }
    });
  }
}
