import 'dart:ui' as ui;

import 'package:flutter/services.dart';

final screenshotChannel = () {
  const channel = MethodChannel('com.memolanes/screenshots');
  // Discard screenshot events while the map has no listener.
  ui.channelBuffers
    ..resize(channel.name, 0)
    ..allowOverflow(channel.name, true);
  return channel;
}();

Future<ui.Image> decodeScreenshot(Uint8List bytes) async {
  final codec = await ui.instantiateImageCodec(bytes);
  try {
    return (await codec.getNextFrame()).image;
  } finally {
    codec.dispose();
  }
}
