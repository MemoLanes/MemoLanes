import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:file_saver/file_saver.dart';
import 'package:flutter/services.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:share_plus/share_plus.dart';

String _screenshotName() =>
    'MemoLanes_${DateTime.now().millisecondsSinceEpoch}';

Future<void> saveScreenshotToGallery(Uint8List png) async {
  final isAndroid = Platform.isAndroid;
  if (isAndroid &&
      (await DeviceInfoPlugin().androidInfo).version.sdkInt <= 28 &&
      !(await Permission.storage.request()).isGranted) {
    throw PlatformException(
      code: 'permission_denied',
      message: 'Photo permission denied',
    );
  }
  final asset = await FileSaver.instance.saveToGallery(
    name: _screenshotName(),
    bytes: png,
    fileExtension: 'png',
    mimeType: MimeType.png,
    album: isAndroid ? 'MemoLanes' : null,
  );
  if (asset == null) throw StateError('Gallery save returned no asset');
}

Future<void> shareScreenshot(Uint8List png, Rect origin) async {
  await SharePlus.instance.share(
    ShareParams(
      files: [XFile.fromData(png, mimeType: 'image/png')],
      fileNameOverrides: ['${_screenshotName()}.png'],
      sharePositionOrigin: origin,
    ),
  );
}
