import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/duo_device.dart';

import 'support/duo_hardware_test_data.dart';

void main() {
  testWidgets(
    'keyboard and native layout changes keep the map size and draft',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final frames = ValueNotifier(const _Frame());
      addTearDown(frames.dispose);
      await tester.pumpWidget(
        MaterialApp(
          builder: (_, child) => ValueListenableBuilder<_Frame>(
            valueListenable: frames,
            child: child,
            builder: (_, frame, child) => MediaQuery(
              data: frame.media,
              child: DuoHardwareScope(
                hardware: frame.duo
                    ? const DuoHardware(
                        hasHinge: true,
                        status: DuoHingeStatus.partiallyOpen,
                        folds: [DuoRegion(Rect.fromLTWH(460, 0, 20, 669))],
                      )
                    : DuoHardware.none,
                child: DuoBarScope(
                  edge: frame.duo ? DuoBarEdge.right : DuoBarEdge.unknown,
                  child: child!,
                ),
              ),
            ),
          ),
          home: ValueListenableBuilder<_Frame>(
            valueListenable: frames,
            builder: (_, frame, _) => AppNavigationShell(
              contentMode: frame.mode,
              navigationVisible: frame.navigationVisible,
              navigationBuilder: (_, {required axis}) =>
                  const SizedBox.expand(key: Key('navigation')),
              bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
                  _Draft(
                    padding: frame.mode == AppNavigationContentMode.inset
                        ? EdgeInsets.only(
                            top: topSafeArea,
                            bottom: bottomSafeArea,
                          )
                        : EdgeInsets.zero,
                  ),
            ),
          ),
        ),
      );
      final body = tester.state<_DraftState>(find.byType(_Draft));
      await tester.enterText(find.byType(TextField), 'kept draft');
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      body.scroll.jumpTo(140);

      for (final frame in [
        const _Frame(keyboard: 320),
        const _Frame(keyboard: 320, mode: AppNavigationContentMode.inset),
        const _Frame(windowSize: Size(1200, 800)),
        const _Frame(
          duo: true,
          keyboard: 280,
          mode: AppNavigationContentMode.inset,
        ),
        const _Frame(duo: true, keyboard: 280),
        const _Frame(duo: true, navigationVisible: false),
        const _Frame(),
      ]) {
        tester.view.physicalSize = frame.size;
        frames.value = frame;
        await tester.pumpAndSettle();
        expect(tester.state(find.byType(_Draft)), same(body));
        expect(body.note.text, 'kept draft');
        expect(body.scroll.offset, 140);
        expect(
          find.byKey(const Key('navigation')),
          frame.navigationVisible ? findsOneWidget : findsNothing,
        );
        final size = tester.getSize(find.byType(_Draft));
        if (frame.mode == AppNavigationContentMode.fullBleed) {
          expect(size, frame.size);
        } else {
          expect(size.height, frame.size.height - frame.keyboard);
          final layout = AppLayoutScope.of(tester.element(find.byType(_Draft)));
          body.scroll.jumpTo(body.scroll.position.maxScrollExtent);
          await tester.pump();
          expect(
            tester.getBottomRight(find.byKey(const Key('last-item'))).dy,
            lessThanOrEqualTo(layout.contentBounds.bottom),
          );
          body.scroll.jumpTo(140);
          await tester.pump();
        }
        expect(tester.takeException(), isNull);
      }
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );
  testWidgets('nested containers consume insets and margins once', (
    tester,
  ) async {
    Widget content({required bool nested}) => AppLayoutBuilder(
      navigationVisible: true,
      builder: (_, layout) => PageContentInsets.menu(
        layout: layout,
        child: PageContentFrame(
          consumeChildMargins: true,
          child: nested
              ? const PageContentInsets(
                  child: SafeAreaWrapper(
                    child: SizedBox.expand(key: Key('probe')),
                  ),
                )
              : const SafeArea(child: SizedBox.expand(key: Key('probe'))),
        ),
      ),
    );
    for (final duo in [true, false]) {
      await _pumpInsets(tester, content(nested: false), duo: duo);
      final bounds = tester.getRect(find.byKey(const Key('probe')));
      await _pumpInsets(tester, content(nested: true), duo: duo);
      expect(tester.getRect(find.byKey(const Key('probe'))), bounds);
      expect(
        MediaQuery.paddingOf(tester.element(find.byKey(const Key('probe')))),
        EdgeInsets.zero,
      );
      expect(tester.takeException(), isNull);
    }
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets('opting out of framing preserves ordinary SafeArea behavior', (
    tester,
  ) async {
    const content = SafeAreaWrapper(child: SizedBox.expand(key: Key('probe')));
    await _pumpInsets(tester, content, duo: false);
    final bounds = tester.getRect(find.byKey(const Key('probe')));
    await _pumpInsets(
      tester,
      const PageContentFrame(framed: false, child: content),
      duo: false,
    );
    expect(tester.getRect(find.byKey(const Key('probe'))), bounds);
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.android}));
}

Future<void> _pumpInsets(
  WidgetTester tester,
  Widget child, {
  required bool duo,
}) async {
  const size = Size(951, 669);
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final padding = duo
      ? const EdgeInsets.only(right: 84, bottom: 34)
      : const EdgeInsets.fromLTRB(11, 59, 17, 34);
  await tester.pumpWidget(
    MaterialApp(
      home: MediaQuery(
        data: MediaQueryData(
          size: size,
          padding: padding,
          viewPadding: padding,
          viewInsets: EdgeInsets.only(bottom: duo ? 246 : 0),
        ),
        child: DuoHardwareScope(
          hardware: duo
              ? const DuoHardware(
                  hasHinge: true,
                  status: DuoHingeStatus.fullyOpen,
                )
              : DuoHardware.none,
          child: DuoBarScope(
            edge: duo ? DuoBarEdge.right : DuoBarEdge.unknown,
            child: Scaffold(body: child),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

class _Frame {
  const _Frame({
    this.duo = false,
    this.keyboard = 0,
    this.navigationVisible = true,
    this.mode = AppNavigationContentMode.fullBleed,
    this.windowSize = const Size(393, 852),
  });
  final bool duo;
  final double keyboard;
  final bool navigationVisible;
  final AppNavigationContentMode mode;
  final Size windowSize;
  Size get size => duo ? const Size(951, 669) : windowSize;
  MediaQueryData get media => MediaQueryData(
    size: size,
    padding: EdgeInsets.only(
      right: duo ? 84 : 0,
      bottom: keyboard > 0 ? 0 : 34,
    ),
    viewPadding: EdgeInsets.only(right: duo ? 84 : 0, bottom: 34),
    viewInsets: EdgeInsets.only(bottom: keyboard),
  );
}

class _Draft extends StatefulWidget {
  const _Draft({required this.padding});
  final EdgeInsets padding;
  @override
  State<_Draft> createState() => _DraftState();
}

class _DraftState extends State<_Draft> {
  final note = TextEditingController();
  final scroll = ScrollController();
  @override
  void dispose() {
    note.dispose();
    scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListView(
    controller: scroll,
    padding: widget.padding,
    children: [
      TextField(controller: note),
      const SizedBox(key: Key('last-item'), height: 1600),
    ],
  );
}
