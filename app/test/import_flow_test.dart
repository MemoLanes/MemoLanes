import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/utils.dart';

void main() {
  testWidgets(
    'waits for the final preview after the loading route is replaced',
    (tester) async {
      final key = GlobalKey<NavigatorState>();
      await tester.pumpWidget(
        MaterialApp(navigatorKey: key, home: const Scaffold()),
      );

      final flow = ImportFlow();
      var entryClosed = false;
      var importClosed = false;
      final entry = key.currentState!.push<void>(
        MaterialPageRoute(
          builder: (_) => const Scaffold(body: Text('Loading')),
        ),
      );
      entry.then((_) => entryClosed = true);
      final completion = flow.waitFor(entry).then((_) => importClosed = true);
      await tester.pumpAndSettle();

      final preview = key.currentState!.pushReplacement<bool, void>(
        MaterialPageRoute(
          builder: (_) => const Scaffold(body: Text('Preview')),
        ),
      );
      flow.continueWith(preview);
      await tester.pumpAndSettle();
      expect(entryClosed, isTrue);
      expect(importClosed, isFalse);

      key.currentState!.push<void>(
        MaterialPageRoute(
          builder: (_) => const Scaffold(body: Text('Details')),
        ),
      );
      await tester.pumpAndSettle();
      key.currentState!.pop();
      await tester.pumpAndSettle();
      expect(importClosed, isFalse);

      key.currentState!.pop(true);
      await tester.pumpAndSettle();
      await completion;
      expect(importClosed, isTrue);
    },
  );

  testWidgets('finishes when an entry route closes without replacement', (
    tester,
  ) async {
    final key = GlobalKey<NavigatorState>();
    await tester.pumpWidget(
      MaterialApp(navigatorKey: key, home: const Scaffold()),
    );
    final flow = ImportFlow();
    var importClosed = false;
    final completion = flow
        .waitFor(
          key.currentState!.push<void>(
            MaterialPageRoute(builder: (_) => const Scaffold()),
          ),
        )
        .then((_) => importClosed = true);
    await tester.pumpAndSettle();
    expect(importClosed, isFalse);
    key.currentState!.pop();
    await tester.pumpAndSettle();
    await completion;
    expect(importClosed, isTrue);
  });
}
