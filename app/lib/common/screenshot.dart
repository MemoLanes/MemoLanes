import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

Future<ui.Image> decodeScreenshot(Uint8List bytes) async {
  final codec = await ui.instantiateImageCodec(bytes);
  try {
    return (await codec.getNextFrame()).image;
  } finally {
    codec.dispose();
  }
}

/// Captures Flutter content only. Never wrap the platform WebView in this.
Future<ui.Image> captureScreenshotBoundary(
  GlobalKey key,
  double pixelRatio,
) async {
  await WidgetsBinding.instance.endOfFrame;
  final boundary = key.currentContext?.findRenderObject();
  if (boundary is! RenderRepaintBoundary || !boundary.hasSize) {
    throw StateError('Screenshot content is unavailable');
  }
  return boundary.toImage(pixelRatio: pixelRatio);
}
