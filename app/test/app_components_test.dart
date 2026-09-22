import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  testWidgets('loading button cannot trigger duplicate actions', (
    tester,
  ) async {
    var pressCount = 0;

    Widget buildButton({required bool loading}) {
      return MaterialApp(
        theme: AppTheme.dark,
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
        theme: AppTheme.light,
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
                      child: const Text('This action cannot be undone.'),
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

  testWidgets('an open popup inherits changes to the active theme', (
    tester,
  ) async {
    Widget buildApp(ThemeData theme) {
      return MaterialApp(
        theme: theme,
        home: Scaffold(
          body: Center(
            child: CustomPopup(
              contentBuilder: (context) => const Text('Popup content'),
              child: const Text('Open popup'),
            ),
          ),
        ),
      );
    }

    await tester.pumpWidget(buildApp(AppTheme.light));
    await tester.tap(find.text('Open popup'));
    await tester.pumpAndSettle();
    final content = find.text('Popup content');
    expect(content, findsOneWidget);
    expect(Theme.of(tester.element(content)).brightness, Brightness.light);

    await tester.pumpWidget(buildApp(AppTheme.dark));
    await tester.pumpAndSettle();
    expect(content, findsOneWidget);
    expect(Theme.of(tester.element(content)).brightness, Brightness.dark);

    await tester.pumpWidget(buildApp(AppTheme.light));
    await tester.pumpAndSettle();
    expect(content, findsOneWidget);
    expect(Theme.of(tester.element(content)).brightness, Brightness.light);
  });
}
