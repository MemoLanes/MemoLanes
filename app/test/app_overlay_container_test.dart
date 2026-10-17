import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_menu.dart';
import 'package:memolanes/common/component/map_controls/map_popup_surface.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

import 'support/duo_hardware_test_data.dart';

const _ios = TargetPlatformVariant({TargetPlatform.iOS});

void _view(WidgetTester tester, Size size) {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
}

Future<void> _pump(WidgetTester tester, Widget home, {ThemeData? theme}) async {
  await tester.pumpWidget(
    MaterialApp(theme: theme ?? AppTheme.light, home: home),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'dialogs retain reachable actions across phone rotation and keyboard changes',
    (tester) async {
      _view(tester, const Size(900, 600));
      addTearDown(tester.view.resetPadding);
      addTearDown(tester.view.resetViewPadding);
      addTearDown(tester.view.resetViewInsets);
      bool? result;
      await _pump(
        tester,
        Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () async => result = await showAppDialog<bool>(
                context,
                barrierDismissible: false,
                builder: (context) => AppDialogContent(
                  title: 'Form',
                  actions: TextButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: const Text('Done'),
                  ),
                  child: Column(
                    children: [for (var i = 0; i < 60; i++) Text('Field $i')],
                  ),
                ),
              ),
              child: const Text('Open'),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Open'));
      await tester.pumpAndSettle();
      await tester.tapAt(const Offset(4, 4));
      await tester.pumpAndSettle();
      expect(
        find.byWidgetPredicate((w) => w is WoltModalSheet),
        findsOneWidget,
      );
      final scroll = find.descendant(
        of: find.byType(AppDialogContent),
        matching: find.byType(SingleChildScrollView),
      );
      await tester.drag(scroll, const Offset(0, -180));
      await tester.pumpAndSettle();
      expect(find.text('Form').hitTestable(), findsOneWidget);
      final wide = tester.getRect(find.byType(AppDialogSurface));
      for (final (size, padding, keyboard) in const [
        (Size(393, 852), FakeViewPadding(top: 47, bottom: 34), 0.0),
        (Size(852, 393), FakeViewPadding(left: 47, bottom: 21), 0.0),
        (Size(393, 852), FakeViewPadding(top: 47, bottom: 34), 320.0),
        (Size(360, 320), FakeViewPadding(), 0.0),
      ]) {
        tester.view.physicalSize = size;
        tester.view.viewPadding = padding;
        tester.view.padding = FakeViewPadding(
          left: padding.left,
          top: padding.top,
          right: padding.right,
          bottom: keyboard > 0 ? 0 : padding.bottom,
        );
        tester.view.viewInsets = FakeViewPadding(bottom: keyboard);
        await tester.pumpAndSettle();
        final bounds = tester.getRect(find.byType(AppDialogSurface));
        expect(bounds.width, lessThanOrEqualTo(wide.width));
        expect(bounds.center.dx, closeTo(size.width / 2, 0.01));
        expect(bounds.left, greaterThanOrEqualTo(padding.left));
        expect(bounds.right, lessThanOrEqualTo(size.width - padding.right));
        expect(bounds.top, greaterThanOrEqualTo(padding.top));
        expect(
          bounds.bottom,
          lessThanOrEqualTo(
            size.height - (keyboard > 0 ? keyboard : padding.bottom),
          ),
        );
        expect(find.text('Done').hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
      await tester.tap(find.text('Done'));
      await tester.pumpAndSettle();
      expect(find.byType(AppDialogSurface), findsNothing);
      expect(result, isTrue);
      expect(tester.takeException(), isNull);
    },
    variant: _ios,
  );

  testWidgets('ordinary foldable dialogs keep the engine pane', (tester) async {
    const size = Size(800, 600);
    const hinge = Rect.fromLTWH(390, 0, 20, 600);
    _view(tester, size);
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light,
        builder: (_, child) => Directionality(
          textDirection: TextDirection.ltr,
          child: MediaQuery(
            data: const MediaQueryData(
              size: size,
              displayFeatures: [
                ui.DisplayFeature(
                  bounds: hinge,
                  type: ui.DisplayFeatureType.hinge,
                  state: ui.DisplayFeatureState.postureHalfOpened,
                ),
              ],
            ),
            child: child!,
          ),
        ),
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => showAppDialog<void>(
                context,
                builder: (_) => const AppDialogContent(
                  title: 'Native foldable',
                  child: Text('Keep the existing pane'),
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
    final bounds = tester.getRect(find.byType(AppDialogSurface));
    expect(bounds.left, greaterThanOrEqualTo(0));
    expect(bounds.right, lessThanOrEqualTo(hinge.left));
    expect(bounds.overlaps(hinge), isFalse);
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.android}));

  testWidgets('the modal retains its caller theme above the root Navigator', (
    tester,
  ) async {
    _view(tester, const Size(393, 852));
    await _pump(
      tester,
      Theme(
        data: AppTheme.dark,
        child: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => showAppDialog<void>(
                context,
                builder: (_) => const AppDialogContent(
                  title: 'Local theme',
                  child: Text('Retain the existing colors'),
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
    final context = tester.element(find.byType(AppDialogContent));
    expect(Theme.of(context).brightness, Brightness.dark);
    expect(
      context.appColors.canvasColor,
      AppTheme.dark.extension<AppColors>()!.canvasColor,
    );
    expect(tester.takeException(), isNull);
  }, variant: _ios);

  testWidgets(
    'MenuAnchor preserves bottom attachment and consumes outside taps',
    (tester) async {
      _view(tester, const Size(800, 600));
      tester.view.padding = const FakeViewPadding(left: 84);
      tester.view.viewPadding = const FakeViewPadding(left: 84);
      addTearDown(tester.view.resetPadding);
      addTearDown(tester.view.resetViewPadding);
      var backgroundTaps = 0;
      await _pump(
        tester,
        Scaffold(
          body: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => backgroundTaps++,
            child: SafeArea(
              child: Align(
                alignment: Alignment.centerLeft,
                child: AppMenu(
                  position: AppMenuPosition.bottom,
                  offset: const Offset(0, 8),
                  surfaceBuilder: (_, child) =>
                      AppMenuSurface(padding: EdgeInsets.zero, child: child),
                  menuBuilder: (_) => const SizedBox(
                    width: 160,
                    height: 80,
                    child: Text('Menu content'),
                  ),
                  child: const SizedBox(
                    width: 40,
                    height: 40,
                    child: Text('Open menu'),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Open menu'));
      await tester.pumpAndSettle();
      final expected = tester.getRect(find.byType(AppMenuSurface));
      expect(expected.left, greaterThanOrEqualTo(84));
      // A point in the shadow must dismiss and consume the tap.
      await tester.tapAt(expected.topLeft - const Offset(2, 2));
      await tester.pumpAndSettle();
      expect(find.text('Menu content'), findsNothing);
      expect(backgroundTaps, 0);
      await tester.tapAt(const Offset(20, 20));
      expect(backgroundTaps, 1);
      expect(tester.takeException(), isNull);
    },
    variant: _ios,
  );

  for (final (pane, onLeft, direction) in [
    (0.0, true, TextDirection.ltr),
    (410.0, false, TextDirection.rtl),
  ]) {
    testWidgets(
      'side menu opens inward in pane $pane onLeft:$onLeft $direction',
      (tester) async {
        _view(tester, const Size(800, 600));
        const anchorKey = Key('side-menu-anchor');
        await _pump(
          tester,
          DuoHardwareScope(
            hardware: const DuoHardware(
              supported: true,
              hasHinge: true,
              status: DuoHingeStatus.partiallyOpen,
              folds: [DuoRegion(Rect.fromLTWH(390, 0, 20, 600))],
            ),
            bridge: DuoBridge.none,
            child: DuoBarScope(
              edge: DuoBarEdge.horizontal,
              child: Directionality(
                textDirection: direction,
                child: Scaffold(
                  body: Stack(
                    children: [
                      Positioned(
                        left: pane + (onLeft ? 16 : 334),
                        top: 250,
                        child: AppMenu(
                          position: AppMenuPosition.side,
                          offset: const Offset(16, 0),
                          menuBuilder: (_) => const SizedBox(
                            width: 160,
                            height: 80,
                            child: Text('Side menu content'),
                          ),
                          child: const SizedBox(
                            key: anchorKey,
                            width: 40,
                            height: 40,
                            child: Text('Open side menu'),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.tap(find.byKey(anchorKey));
        await tester.pumpAndSettle();
        final anchor = tester.getRect(find.byKey(anchorKey));
        final menu = tester.getRect(find.byType(AppMenuSurface));
        expect(
          onLeft ? menu.left - anchor.right : anchor.left - menu.right,
          16,
        );
        expect(menu.overlaps(anchor), isFalse);
        expect(menu.overlaps(const Rect.fromLTWH(390, 0, 20, 600)), isFalse);
        expect(tester.takeException(), isNull);
      },
      variant: _ios,
    );
  }

  testWidgets('menus release route history after Escape, resize and Back', (
    tester,
  ) async {
    _view(tester, const Size(393, 852));
    final navigator = GlobalKey<NavigatorState>();
    final menu = MenuController();
    await tester.pumpWidget(
      MaterialApp(
        navigatorKey: navigator,
        theme: AppTheme.light,
        home: const Scaffold(body: Text('Home')),
      ),
    );
    navigator.currentState!.push(
      MaterialPageRoute<void>(
        builder: (_) => Scaffold(
          body: AppMenu(
            controller: menu,
            menuBuilder: (_) => const Text('Open menu content'),
            child: const Text('Menu page'),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final route = ModalRoute.of(tester.element(find.text('Menu page')))!;
    for (final resize in [false, true]) {
      menu.open();
      await tester.pumpAndSettle();
      if (resize) {
        tester.view.physicalSize = const Size(852, 393);
      } else {
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      }
      await tester.pumpAndSettle();
      expect(menu.isOpen, isFalse);
      expect(route.willHandlePopInternally, isFalse);
    }
    menu.open();
    menu.open();
    await tester.pumpAndSettle();
    expect(menu.isOpen, isTrue);
    await navigator.currentState!.maybePop();
    await tester.pumpAndSettle();
    expect(menu.isOpen, isFalse);
    expect(find.text('Menu page'), findsOneWidget);
    await navigator.currentState!.maybePop();
    await tester.pumpAndSettle();
    expect(find.text('Home'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, variant: _ios);

  testWidgets('a menu can close under a modal without popping that modal', (
    tester,
  ) async {
    _view(tester, const Size(393, 852));
    final menu = MenuController();
    await _pump(
      tester,
      Scaffold(
        body: Center(
          child: AppMenu(
            controller: menu,
            menuBuilder: (context) => TextButton(
              onPressed: () => showAppDialog<void>(
                context,
                builder: (context) => AppDialogContent(
                  title: 'Keep this modal',
                  child: TextButton(
                    onPressed: () => Navigator.pop(context),
                    child: const Text('Close modal'),
                  ),
                ),
              ),
              child: const Text('Open modal'),
            ),
            child: const Text('Open menu'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open menu'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Open modal'));
    await tester.pumpAndSettle();
    menu.close();
    await tester.pumpAndSettle();
    expect(find.text('Keep this modal'), findsOneWidget);
    await tester.tap(find.text('Close modal'));
    await tester.pumpAndSettle();
    expect(find.text('Open menu'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, variant: _ios);

  testWidgets('the anchor can reopen its menu during closing', (tester) async {
    _view(tester, const Size(800, 600));
    final menu = MenuController();
    await _pump(
      tester,
      Scaffold(
        body: Center(
          child: AppMenu(
            controller: menu,
            menuBuilder: (_) => const Text('Menu content'),
            child: const Text('Open menu'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open menu'));
    await tester.pumpAndSettle();
    menu.close();
    await tester.pump(const Duration(milliseconds: 120));
    await tester.tap(find.text('Open menu'));
    await tester.pumpAndSettle();
    expect(find.text('Menu content'), findsOneWidget);
    menu.close();
    await tester.pumpAndSettle();
    expect(find.text('Menu content'), findsNothing);
  }, variant: _ios);

  testWidgets('a long menu clears the keyboard and scrolls inside its glass', (
    tester,
  ) async {
    _view(tester, const Size(800, 600));
    tester.view.viewInsets = const FakeViewPadding(bottom: 240);
    addTearDown(tester.view.resetViewInsets);
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light,
        home: Scaffold(
          body: Center(
            child: AppMenu(
              position: AppMenuPosition.side,
              surfaceBuilder: (_, child) => MapPopupSurface(child: child),
              menuBuilder: (_) => const SizedBox(
                width: 160,
                height: 900,
                child: Text('Long content'),
              ),
              child: const Text('Open menu'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open menu'));
    await tester.pumpAndSettle();
    final surface = find.byType(MapPopupSurface);
    final bounds = tester.getRect(surface);
    expect(bounds.top, greaterThanOrEqualTo(0));
    expect(bounds.bottom, lessThanOrEqualTo(360));
    final bodyScroll = find.descendant(
      of: surface,
      matching: find.byType(Scrollable),
    );
    expect(
      tester.state<ScrollableState>(bodyScroll).position.maxScrollExtent,
      greaterThan(0),
    );
    await tester.drag(bodyScroll, const Offset(0, -300));
    await tester.pumpAndSettle();
    expect(find.text('Long content'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
