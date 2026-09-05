import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';

void main() {
  testWidgets('loading button cannot trigger duplicate actions', (
    tester,
  ) async {
    var pressCount = 0;

    Widget buildButton({required bool loading}) {
      return MaterialApp(
        theme: ThemeData.dark(),
        home: Scaffold(
          body: Center(
            child: AppButton(
              label: 'Save',
              loading: loading,
              onPressed: () => pressCount++,
            ),
          ),
        ),
      );
    }

    await tester.pumpWidget(buildButton(loading: true));

    await tester.tap(find.text('Save'));
    await tester.tap(find.text('Save'));
    expect(pressCount, 0);

    await tester.pumpWidget(buildButton(loading: false));
    await tester.tap(find.text('Save'));
    expect(pressCount, 1);
  });

  testWidgets('dialog actions close the dialog and return the chosen result', (
    tester,
  ) async {
    final results = <bool?>[];

    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () async {
                results.add(
                  await showAppDialog<bool>(
                    context,
                    barrierDismissible: false,
                    builder: (dialogContext) => AppDialogCard(
                      title: 'Delete item?',
                      child: const Text('This action cannot be undone.'),
                      actions: AppDialogActions(
                        children: [
                          AppButton(
                            label: 'Cancel',
                            onPressed: () =>
                                Navigator.of(dialogContext).pop(false),
                          ),
                          AppButton(
                            label: 'Delete',
                            variant: AppButtonVariant.danger,
                            onPressed: () =>
                                Navigator.of(dialogContext).pop(true),
                          ),
                        ],
                      ),
                    ),
                  ),
                );
              },
              child: const Text('Open'),
            ),
          ),
        ),
      ),
    );

    for (final action in ['Cancel', 'Delete']) {
      await tester.tap(find.text('Open'));
      await tester.pumpAndSettle();
      await tester.tap(find.text(action));
      await tester.pumpAndSettle();
      expect(find.text('Delete item?'), findsNothing);
      expect(find.text('Open').hitTestable(), findsOneWidget);
    }

    expect(results, [false, true]);
  });
}
