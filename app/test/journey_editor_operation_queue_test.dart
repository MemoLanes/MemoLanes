import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/editor/journey_editor_operation_queue.dart';

void main() {
  test('retains consecutive inputs and stays busy until all finish', () async {
    final queue = JourneyEditorOperationQueue();
    addTearDown(queue.dispose);
    final firstGate = Completer<void>();
    final lastGate = Completer<void>();
    final mutations = <String>[];
    final busyStates = <bool>[];
    queue.addListener(() => busyStates.add(queue.isBusy));

    final first = queue.run(() async {
      mutations.add('first stroke');
      await firstGate.future;
    });
    final second = queue.run(() async => mutations.add('second stroke'));
    final third = queue.run(() async {
      mutations.add('erase');
      await lastGate.future;
    });
    expect(queue.isBusy, isTrue);
    await Future<void>.delayed(Duration.zero);
    expect(mutations, ['first stroke']);

    firstGate.complete();
    await second;
    await Future<void>.delayed(Duration.zero);
    expect(mutations, ['first stroke', 'second stroke', 'erase']);
    expect(queue.isBusy, isTrue);
    expect(busyStates, everyElement(isTrue));

    lastGate.complete();
    await Future.wait([first, second, third]);
    expect(queue.isBusy, isFalse);
    expect(busyStates.last, isFalse);
  });

  test('reports a failed operation and continues with later input', () async {
    final queue = JourneyEditorOperationQueue();
    addTearDown(queue.dispose);
    var nextOperationRan = false;
    final failed = queue.run(() async => throw StateError('native failure'));
    final failure = expectLater(failed, throwsStateError);
    final next = queue.run(() async => nextOperationRan = true);

    await failure;
    await next;
    expect(nextOperationRan, isTrue);
    expect(queue.isBusy, isFalse);
  });

  test(
    'leaving the editor skips waiting mutations without notifications',
    () async {
      final queue = JourneyEditorOperationQueue();
      final gate = Completer<void>();
      var waitingOperationRan = false;
      var notifications = 0;
      queue.addListener(() => notifications++);
      final first = queue.run(() => gate.future);
      final next = queue.run(() async => waitingOperationRan = true);
      await Future<void>.delayed(Duration.zero);

      queue.dispose();
      final notificationsAtExit = notifications;
      gate.complete();
      await Future.wait([first, next]);
      expect(waitingOperationRan, isFalse);
      expect(notifications, notificationsAtExit);
      expect(queue.isBusy, isFalse);
    },
  );
}
