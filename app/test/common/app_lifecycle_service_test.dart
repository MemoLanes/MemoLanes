import 'dart:async';

import 'package:flutter_fgbg/flutter_fgbg.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/app_lifecycle_service.dart';

void main() {
  test('serializes foreground and background provider hooks', () async {
    final events = StreamController<FGBGType>();
    final releaseBackground = Completer<void>();
    final calls = <String>[];
    final service = AppLifecycleService(
      events: events.stream,
      reloadResource: () async => calls.add('reload'),
    );
    service.start(
      onBackground: () async {
        calls.add('background-start');
        await releaseBackground.future;
        calls.add('background-end');
      },
      onForeground: () async => calls.add('foreground'),
    );

    events.add(FGBGType.background);
    events.add(FGBGType.foreground);
    await Future<void>.delayed(Duration.zero);
    expect(calls, ['background-start']);

    releaseBackground.complete();
    await service.stop();
    expect(calls, [
      'background-start',
      'background-end',
      'reload',
      'foreground',
    ]);
    await events.close();
  });

  test('a failed lifecycle hook does not break later work', () async {
    final events = StreamController<FGBGType>();
    final foregroundDone = Completer<void>();
    final service = AppLifecycleService(
      events: events.stream,
      reloadResource: () async {},
    );
    service.start(
      onBackground: () async => throw StateError('test failure'),
      onForeground: () async => foregroundDone.complete(),
    );

    events.add(FGBGType.background);
    events.add(FGBGType.foreground);
    await foregroundDone.future;

    await service.stop();
    await events.close();
  });
}
