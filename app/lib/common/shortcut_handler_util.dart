import 'dart:async';

import 'package:intelligence/intelligence.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';

class ShortcutHandlerUtil {
  ShortcutHandlerUtil._();

  static StreamSubscription<String>? _selectionSub;

  static void init({required GpsManager gpsManager}) {
    final previousSub = _selectionSub;
    if (previousSub != null) {
      log.warning(
          'ShortcutHandlerUtil.init called more than once, cancelling previous subscription.');
      previousSub.cancel();
    }
    _selectionSub = Intelligence().selectionsStream().listen(
      (selection) async {
        try {
          switch (selection) {
            case 'start_recording':
              await gpsManager
                  .changeRecordingState(GpsRecordingStatus.recording);
              break;
            case 'stop_recording':
              await gpsManager.changeRecordingState(GpsRecordingStatus.none);
              break;
            case 'pause_recording':
              await gpsManager.changeRecordingState(GpsRecordingStatus.paused);
              break;
            default:
              log.warning('Unknown shortcut selection: $selection');
          }
        } catch (e, s) {
          log.error('Failed to handle shortcut selection: $selection', s);
        }
      },
      onError: (Object err) {
        log.error('Error in shortcut selection stream: $err');
      },
    );
  }
}
