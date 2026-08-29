import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/common_dialog.dart';

void main() {
  testWidgets('loading button cannot trigger duplicate actions',
      (tester) async {
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

    final loadingButton = tester.widget<FilledButton>(
      find.byType(FilledButton),
    );
    expect(loadingButton.onPressed, isNull);
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    await tester.tap(find.text('Save'));
    expect(pressCount, 0);

    await tester.pumpWidget(buildButton(loading: false));
    await tester.tap(find.text('Save'));
    expect(pressCount, 1);
  });

  testWidgets('dialog card stays usable on a narrow viewport', (tester) async {
    tester.view.physicalSize = const Size(320, 480);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData.dark(),
        home: Builder(
          builder: (context) => Scaffold(
            body: Center(
              child: TextButton(
                onPressed: () => showAppDialog<void>(
                  context,
                  child: AppDialogCard(
                    title: 'A long but readable dialog title',
                    actions: AppDialogActions(
                      children: [
                        AppButton(
                          label: 'Cancel operation',
                          variant: AppButtonVariant.secondary,
                          onPressed: () => Navigator.of(context).pop(),
                        ),
                        AppButton(
                          label: 'Confirm operation',
                          onPressed: () => Navigator.of(context).pop(),
                        ),
                      ],
                    ),
                    child:
                        Text(List.filled(30, 'Scrollable content').join('\n')),
                  ),
                ),
                child: const Text('Open'),
              ),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byType(Dialog), findsOneWidget);
    expect(find.textContaining('Scrollable content'), findsOneWidget);
    expect(find.text('Cancel operation'), findsOneWidget);
    expect(find.text('Confirm operation'), findsOneWidget);
  });

  testWidgets('common dialog returns the selected action', (tester) async {
    bool? result;

    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData.dark(),
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () async {
                result = await showAppDialog<bool>(
                  context,
                  barrierDismissible: false,
                  child: CommonDialog(
                    title: 'Delete item?',
                    content: 'This action cannot be undone.',
                    buttons: [
                      DialogButton(
                        text: 'Cancel',
                        onPressed: () => Navigator.of(context).pop(false),
                      ),
                      DialogButton(
                        text: 'Delete',
                        variant: AppButtonVariant.danger,
                        onPressed: () => Navigator.of(context).pop(true),
                      ),
                    ],
                  ),
                );
              },
              child: const Text('Open'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(find.text('Delete'), findsOneWidget);
    tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .last
        .onPressed();
    await tester.pumpAndSettle();

    expect(result, isTrue);
    expect(find.byType(Dialog), findsNothing);
  });
}
