import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/journey_idle_timer.dart';

void main() {
  testWidgets('a late tick gives buffered fixes one minute to arrive', (
    tester,
  ) async {
    var now = DateTime.utc(2026, 1, 1);
    var checks = 0;
    final timer = JourneyIdleTimer((_) async => checks++, now: () => now);
    timer.restart();

    now = now.add(const Duration(hours: 8));
    await tester.pump(const Duration(minutes: 30));
    expect(checks, 0);

    timer.onResume();
    now = now.add(const Duration(minutes: 1));
    await tester.pump(const Duration(minutes: 1));
    expect(checks, 1);
    timer.cancel();
  });

  testWidgets(
    'resume keeps the deadline or leaves one minute, whichever is later',
    (tester) async {
      var checks = 0;
      final timer = JourneyIdleTimer(
        (_) async => checks++,
        now: tester.binding.clock.now,
      );
      timer.restart();
      await tester.pump(const Duration(minutes: 20));
      timer.onResume();
      await tester.pump(const Duration(minutes: 10));
      expect(checks, 1);

      await tester.pump(const Duration(minutes: 29, seconds: 50));
      timer.onResume();
      await tester.pump(const Duration(seconds: 59));
      expect(checks, 1);
      await tester.pump(const Duration(seconds: 1));
      expect(checks, 2);
      timer.cancel();
    },
  );

  testWidgets('resume invalidates a check waiting behind recorded points', (
    tester,
  ) async {
    final queueDrained = Completer<void>();
    var checks = 0;
    var finalized = 0;
    final timer = JourneyIdleTimer((isCurrent) async {
      checks++;
      await queueDrained.future;
      if (isCurrent()) finalized++;
    }, now: () => tester.binding.clock.now());
    timer.restart();

    await tester.pump(const Duration(minutes: 30));
    expect(checks, 1);
    timer.onResume();
    queueDrained.complete();
    await tester.pump();
    expect(finalized, 0);

    await tester.pump(const Duration(minutes: 1));
    expect(finalized, 1);
    timer.cancel();
  });
}
