import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

void main() {
  test('Flutter deep-link routing is disabled for Android share intents', () {
    final manifest =
        File('android/app/src/main/AndroidManifest.xml').readAsStringSync();

    expect(
      manifest,
      contains(
        RegExp(
          r'android:name="flutter_deeplinking_enabled"\s*'
          r'android:value="false"',
        ),
      ),
      reason: 'MemoLanes handles incoming share intents with share_handler. '
          'Flutter must not also interpret a shared Intent.data URI as a route.',
    );
  });
}
