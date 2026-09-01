import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';

void main() {
  testWidgets('dark liquid glass omits the scratch-like top highlight', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Center(
          child: SizedBox(
            width: 240,
            height: 58,
            child: LiquidGlassSurface(child: SizedBox.expand()),
          ),
        ),
      ),
    );

    final hasNarrowTopHighlight = tester
        .widgetList<Positioned>(
          find.descendant(
            of: find.byType(LiquidGlassSurface),
            matching: find.byType(Positioned),
          ),
        )
        .any((positioned) => positioned.top == 1 && positioned.height == 1.2);

    expect(hasNarrowTopHighlight, isFalse);
    expect(find.byType(BackdropFilter), findsOneWidget);
  });

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

  testWidgets('short dialog shrink-wraps instead of filling its height cap', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData.dark(),
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => showAppDialog<void>(
                context,
                builder: (dialogContext) => CommonDialog(
                  title: 'Information',
                  content: 'A short message.',
                  buttons: [
                    DialogButton(
                      text: 'OK',
                      onPressed: () => Navigator.of(dialogContext).pop(),
                    ),
                  ],
                ),
              ),
              child: const Text('Open'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    final dialogHeight = tester.getSize(find.byType(AppDialogCard)).height;
    expect(dialogHeight, lessThan(400));
    expect(dialogHeight, lessThan(800 * 0.78));
  });

  testWidgets('long dialog stays usable on a narrow viewport', (tester) async {
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
                  builder: (dialogContext) => AppDialogCard(
                    title: 'A long but readable dialog title',
                    actions: AppDialogActions(
                      children: [
                        AppButton(
                          label: 'Cancel operation',
                          labelMaxLines: 2,
                          variant: AppButtonVariant.secondary,
                          onPressed: () => Navigator.of(dialogContext).pop(),
                        ),
                        AppButton(
                          label: 'Confirm operation',
                          labelMaxLines: 2,
                          onPressed: () => Navigator.of(dialogContext).pop(),
                        ),
                      ],
                    ),
                    child: Text(
                      List.filled(30, 'Scrollable content').join('\n'),
                    ),
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
    expect(
      find.descendant(
        of: find.byType(AppDialogActions),
        matching: find.byType(Column),
      ),
      findsOneWidget,
    );
  });

  testWidgets('dialog builder supplies the route context and result', (
    tester,
  ) async {
    bool? result;
    BuildContext? sourceContext;
    BuildContext? routeContext;

    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData.dark(),
        home: Builder(
          builder: (context) {
            sourceContext = context;
            return Scaffold(
              body: TextButton(
                onPressed: () async {
                  result = await showAppDialog<bool>(
                    context,
                    barrierDismissible: false,
                    builder: (dialogContext) {
                      routeContext = dialogContext;
                      return CommonDialog(
                        title: 'Delete item?',
                        content: 'This action cannot be undone.',
                        buttons: [
                          DialogButton(
                            text: 'Cancel',
                            onPressed: () =>
                                Navigator.of(dialogContext).pop(false),
                          ),
                          DialogButton(
                            text: 'Delete',
                            variant: AppButtonVariant.danger,
                            onPressed: () =>
                                Navigator.of(dialogContext).pop(true),
                          ),
                        ],
                      );
                    },
                  );
                },
                child: const Text('Open'),
              ),
            );
          },
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    expect(routeContext, isNotNull);
    expect(routeContext, isNot(same(sourceContext)));
    expect(ModalRoute.of(routeContext!), isNotNull);
    tester
        .widget<CommonDialog>(find.byType(CommonDialog))
        .buttons
        .last
        .onPressed();
    await tester.pumpAndSettle();

    expect(result, isTrue);
    expect(find.byType(Dialog), findsNothing);
  });

  testWidgets('dialog actions stack for large text', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: ThemeData.dark(),
        home: MediaQuery(
          data: const MediaQueryData(textScaler: TextScaler.linear(1.5)),
          child: Scaffold(
            body: SizedBox(
              width: 380,
              child: AppDialogActions(
                children: [
                  AppButton(
                    label: 'Disagree and exit',
                    labelMaxLines: 2,
                    onPressed: () {},
                  ),
                  AppButton(
                    label: 'Continue',
                    labelMaxLines: 2,
                    onPressed: () {},
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );

    expect(
      find.descendant(
        of: find.byType(AppDialogActions),
        matching: find.byType(Column),
      ),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });
}
