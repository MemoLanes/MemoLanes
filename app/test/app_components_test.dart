import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  testWidgets('long form remains scrollable in a short viewport', (
    tester,
  ) async {
    const lastField = Key('last-form-field');
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 300,
              height: 120,
              child: MlSingleChildScrollView(
                mainAxisAlignment: MainAxisAlignment.center,
                padding: EdgeInsets.symmetric(vertical: 16),
                children: [
                  SizedBox(height: 320),
                  TextField(key: lastField),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.scrollUntilVisible(
      find.byKey(lastField),
      80,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.enterText(find.byKey(lastField), 'Draft');
    expect(find.text('Draft'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

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
                    builder: (dialogContext) => AppDialogContent(
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

  for (final (width, scale, direction) in const [
    (200.0, 1.8, TextDirection.rtl),
  ]) {
    testWidgets('value fits a local $width pane at $scale/$direction', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1024, 768);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light,
          home: MediaQuery(
            data: MediaQueryData(
              size: const Size(1024, 768),
              textScaler: TextScaler.linear(scale),
            ),
            child: Directionality(
              textDirection: direction,
              child: Scaffold(
                body: Center(
                  child: SizedBox(
                    width: width,
                    child: LabelTile(
                      label: 'Map style',
                      prefix: const Padding(
                        padding: EdgeInsets.only(right: 12),
                        child: Icon(Icons.map, size: 20),
                      ),
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          const Text('NEW'),
                          const SizedBox(width: 8),
                          Flexible(
                            child: LabelTileContent(
                              content: 'Long descriptive value ' * 8,
                              showArrow: true,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      final tile = tester.getRect(find.byType(LabelTile));
      final value = tester.getRect(find.byType(LabelTileContent));
      final title = tester.getRect(find.text('Map style'));
      expect(value.left, greaterThanOrEqualTo(tile.left));
      expect(value.right, lessThanOrEqualTo(tile.right));
      expect(title.width, greaterThan(0));
      expect(tester.takeException(), isNull);
    });
  }
}
