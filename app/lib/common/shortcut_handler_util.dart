import 'dart:async';
import 'dart:io';

import 'package:flutter_app_intents/flutter_app_intents.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';

class ShortcutHandlerUtil {
  ShortcutHandlerUtil._();

  static const _intentSpecs = <_IntentSpec>[
    _IntentSpec(
      id: 'com.memolanes.StartRecordingIntent',
      title: 'Start Recording',
      description: 'Start recording in MemoLanes',
      status: GpsRecordingStatus.recording,
      okValue: 'Started recording',
    ),
    _IntentSpec(
      id: 'com.memolanes.EndJourneyIntent',
      title: 'End Journey',
      description: 'Stop recording and end the current journey in MemoLanes',
      status: GpsRecordingStatus.none,
      okValue: 'Ended journey',
    ),
    _IntentSpec(
      id: 'com.memolanes.PauseRecordingIntent',
      title: 'Pause Recording',
      description: 'Pause recording in MemoLanes',
      status: GpsRecordingStatus.paused,
      okValue: 'Paused recording',
    ),
  ];

  static GpsManager? _gps;

  static void init({required GpsManager gpsManager}) {
    _gps = gpsManager;
    if (!Platform.isIOS) {
      return;
    }
    unawaited(_registerIntents());
  }

  static Future<void> _registerIntents() async {
    final client = FlutterAppIntentsClient.instance;
    final intentsWithHandlers = {
      for (final spec in _intentSpecs)
        spec.intent: (_) => _handleInvocation(spec),
    };
    await client.registerIntents(intentsWithHandlers);
    await client.updateShortcuts();
  }

  static Future<AppIntentResult> _handleInvocation(_IntentSpec spec) async {
    final gps = _gps;
    if (gps == null) {
      final err = '${spec.id}: GpsManager not bound';
      return AppIntentResult.failed(error: err);
    }
    try {
      await gps.changeRecordingState(spec.status);
      return AppIntentResult.successful(value: spec.okValue);
    } catch (e, s) {
      log.error('${spec.id} failed: $e', s);
      return AppIntentResult.failed(error: '$e');
    }
  }
}

class _IntentSpec {
  const _IntentSpec({
    required this.id,
    required this.title,
    required this.description,
    required this.status,
    required this.okValue,
  });

  final String id;
  final String title;
  final String description;
  final GpsRecordingStatus status;
  final String okValue;

  AppIntent get intent => AppIntentBuilder()
      .identifier(id)
      .title(title)
      .description(description)
      .build();
}
