import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/screenshot/screenshot_preview_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/journey_header.dart';

Future<void> showMapScreenshotPreview(
  BuildContext context, {
  required BaseMapWebviewState? map,
  required bool Function() isActive,
  DateTimeRange? Function()? readTimeRange,
  JourneyHeader? journey,
}) async {
  final loading = GlobalLoadingManager.instance;
  if (loading.isLoading || loading.isNavigationBlocked) {
    Fluttertoast.showToast(msg: context.tr('screenshot.not_ready'));
    return;
  }

  ui.Image? mapImage;
  try {
    final screenshot = await loading.runWithLoading(() async {
      // Read the range after the current map proxy and layout are applied.
      await WidgetsBinding.instance.endOfFrame;
      if (!context.mounted || !isActive()) return null;
      final timeRange = readTimeRange?.call();
      if (readTimeRange != null && timeRange == null) {
        Fluttertoast.showToast(msg: context.tr('screenshot.not_ready'));
        return null;
      }
      if (map == null) throw StateError('Map is unavailable');
      final frame = await map.captureScreenshot();
      final image = await decodeScreenshot(frame.png);
      mapImage = image;
      return (
        image: image,
        logicalSize: frame.logicalSize,
        timeRange: timeRange,
      );
    }, blockNavigation: true);
    if (screenshot == null || !context.mounted || !isActive()) return;
    final route = MaterialPageRoute<bool>(
      builder: (_) => ScreenshotPreviewPage(
        map: screenshot.image,
        logicalSize: screenshot.logicalSize,
        timeRange: screenshot.timeRange,
        journey: journey,
      ),
    );
    final saved = await Navigator.of(context).push(route);
    // RawImage still paints during the outgoing route transition.
    await route.completed;
    if (saved == true && context.mounted) {
      Fluttertoast.showToast(msg: context.tr('screenshot.saved'));
    }
  } catch (error, stackTrace) {
    log.error('Capturing screenshot failed: $error', stackTrace);
    if (context.mounted) {
      Fluttertoast.showToast(msg: context.tr('screenshot.failed'));
    }
  } finally {
    mapImage?.dispose();
  }
}

Future<ui.Image> decodeScreenshot(Uint8List bytes) async {
  final codec = await ui.instantiateImageCodec(bytes);
  try {
    return (await codec.getNextFrame()).image;
  } finally {
    codec.dispose();
  }
}
