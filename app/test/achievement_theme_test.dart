import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  testWidgets('achievement popup stays open across system theme changes', (
    tester,
  ) async {
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.system,
        home: const Scaffold(
          body: Center(
            child: AchievementCardTitleRow(
              title: 'Countries',
              info: 'Achievement details',
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byType(CustomPopup));
    await tester.pumpAndSettle();

    void expectPopupTheme(Brightness brightness) {
      final content = find.text('Achievement details');
      expect(content, findsOneWidget);
      expect(Theme.of(tester.element(content)).brightness, brightness);
    }

    expectPopupTheme(Brightness.light);
    // Keep the existing route open; rebuilding/reopening it would hide the bug.
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    await tester.pumpAndSettle();
    expectPopupTheme(Brightness.dark);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    await tester.pumpAndSettle();
    expectPopupTheme(Brightness.light);
    expect(tester.takeException(), isNull);
  });
}
