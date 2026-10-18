import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

void main() {
  final originalWakelock = wakelockPlusPlatformInstance;
  setUp(() {
    wakelockPlusPlatformInstance = _TestWakelockPlatform();
  });
  tearDown(() => wakelockPlusPlatformInstance = originalWakelock);

  for (final fail in [false, true]) {
    testWidgets(
      'loading blocks input immediately and unlocks after ${fail ? 'failure' : 'success'}',
      (tester) async {
        final navigatorKey = GlobalKey<NavigatorState>();
        var taps = 0;
        await tester.pumpWidget(
          MaterialApp(
            navigatorKey: navigatorKey,
            theme: AppTheme.light,
            builder: (_, child) => GlobalLoadingOverlay(child: child!),
            home: const Scaffold(),
            initialRoute: '/task',
            routes: {
              '/task': (_) => GlobalPopScope(
                child: Scaffold(
                  body: Center(
                    child: TextButton(
                      onPressed: () => taps++,
                      child: const Text('Action'),
                    ),
                  ),
                ),
              ),
            },
          ),
        );
        await tester.pumpAndSettle();

        final loading = GlobalLoadingManager.instance;
        final pending = Completer<void>();
        final task = loading.runWithLoading(
          () => pending.future,
          blockNavigation: true,
        );
        final result = expectLater(task, fail ? throwsStateError : completes);
        try {
          await tester.pump();
          expect(loading.isNavigationBlocked, isTrue);
          expect(loading.isLoading, isFalse);

          await tester.pump(const Duration(milliseconds: 180));
          await tester.tapAt(tester.getCenter(find.text('Action')));
          await navigatorKey.currentState!.maybePop();
          await tester.pump();
          expect(taps, 0);
          expect(find.text('Action'), findsOneWidget);
          expect(loading.isLoading, isFalse);

          await tester.pump(const Duration(milliseconds: 100));
          expect(loading.isLoading, isTrue);
          expect(find.byType(CircularProgressIndicator), findsOneWidget);
          if (fail) {
            pending.completeError(StateError('Task failed'));
          } else {
            pending.complete();
          }
          await result;
          await tester.pumpAndSettle();
          expect(loading.isNavigationBlocked, isFalse);
          expect(loading.isLoading, isFalse);
          expect(find.byType(CircularProgressIndicator), findsNothing);
          await tester.tap(find.text('Action'));
          expect(taps, 1);
          await navigatorKey.currentState!.maybePop();
          await tester.pumpAndSettle();
          expect(find.text('Action'), findsNothing);
        } finally {
          if (!pending.isCompleted) {
            if (fail) {
              pending.completeError(StateError('Task failed'));
            } else {
              pending.complete();
            }
          }
          await result;
        }
      },
    );
  }
}

class _TestWakelockPlatform extends WakelockPlusMacOSPlugin {
  @override
  Future<void> toggle({required bool enable}) async {}
}
