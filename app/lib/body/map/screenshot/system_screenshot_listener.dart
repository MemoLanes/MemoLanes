import 'dart:async';
import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:memolanes/common/log.dart';
import 'package:screenshot_detect/screenshot_detect.dart';

StreamSubscription<void> listenForSystemScreenshots(void Function() onCapture) {
  return _systemScreenshotEvents().listen(
    (_) => onCapture(),
    onError: (Object error, StackTrace stackTrace) {
      log.error('System screenshot detection failed: $error', stackTrace);
    },
  );
}

Stream<void> _systemScreenshotEvents() async* {
  if (Platform.isAndroid) {
    final android = await DeviceInfoPlugin().androidInfo;
    if (android.version.sdkInt < 34) return;
  } else if (!Platform.isIOS) {
    return;
  }
  yield* ScreenshotDetect().onScreenshot;
}
