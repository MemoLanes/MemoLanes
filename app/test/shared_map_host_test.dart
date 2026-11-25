import 'dart:ui' as ui;

import 'package:flutter/rendering.dart' show RenderRepaintBoundary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/component/page_content_layout.dart';

int creations = 0;
int documents = 0;
int callbacks = 0;

class _PlatformMap extends StatefulWidget {
  const _PlatformMap({super.key});
  @override
  State<_PlatformMap> createState() => _PlatformMapState();
}

class _PlatformMapState extends State<_PlatformMap> {
  @override
  void initState() {
    super.initState();
    creations++;
    documents++;
  }

  @override
  Widget build(BuildContext context) => Semantics(
    label: "test globe",
    child: GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => callbacks++,
      child: const ColoredBox(color: Colors.blue, child: SizedBox.expand()),
    ),
  );
}

Future<Color> _pixel(WidgetTester tester, Key boundaryKey, Offset point) async {
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(boundaryKey),
  );
  return (await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 1);
    final bytes = (await image.toByteData(format: ui.ImageByteFormat.rawRgba))!;
    final offset = (point.dy.toInt() * image.width + point.dx.toInt()) * 4;
    final color = Color.fromARGB(
      bytes.getUint8(offset + 3),
      bytes.getUint8(offset),
      bytes.getUint8(offset + 1),
      bytes.getUint8(offset + 2),
    );
    image.dispose();
    return color;
  }))!;
}

void main() {
  testWidgets(
    'hidden globe has an opaque full-viewport dashboard cover and retains dashboard scroll beneath Explorer',
    (tester) async {
      creations = documents = callbacks = 0;
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(400, 800);
      tester.view.padding = const FakeViewPadding(top: 60, bottom: 34);
      tester.view.viewPadding = tester.view.padding;
      addTearDown(tester.view.resetPadding);
      addTearDown(tester.view.resetViewPadding);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      const background = Color(0xffeee9df);
      const boundaryKey = ValueKey('shell pixels');
      const dashboardKey = ValueKey('transparent dashboard');
      final page = ValueNotifier(0);
      addTearDown(page.dispose);
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(scaffoldBackgroundColor: background),
          home: RepaintBoundary(
            key: boundaryKey,
            child: ValueListenableBuilder<int>(
              valueListenable: page,
              builder: (_, index, _) => AppNavigationShell(
                retainedMap: const _PlatformMap(
                  key: ValueKey('retained globe'),
                ),
                mapVisible: index != 1,
                contentMode: index == 0
                    ? AppNavigationContentMode.fullBleed
                    : AppNavigationContentMode.inset,
                navigationBuilder: (_, {required axis}) => const SizedBox(),
                fullBleedOverlay: index == 2
                    ? const Align(
                        alignment: Alignment.bottomCenter,
                        child: Text('Explorer overlay'),
                      )
                    : null,
                bodyBuilder:
                    ({required topSafeArea, required bottomSafeArea}) =>
                        index == 0
                        ? const SizedBox.expand()
                        : PageContentLayout.tab(
                            key: dashboardKey,
                            topSafeArea: topSafeArea,
                            bottomSafeArea: bottomSafeArea,
                            children: [
                              for (var i = 0; i < 30; i++)
                                SizedBox(
                                  height: 120,
                                  child: Text('Dashboard row $i'),
                                ),
                            ],
                          ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final mapState = tester.state(find.byType(_PlatformMap));
      expect(
        await _pixel(tester, boundaryKey, const Offset(1, 300)),
        const Color(0xff2196f3),
      );
      page.value = 1;
      await tester.pumpAndSettle();
      for (final point in [
        const Offset(1, 1),
        const Offset(200, 1),
        const Offset(1, 300),
        const Offset(399, 799),
      ]) {
        expect(
          await _pixel(tester, boundaryKey, point),
          background,
          reason: 'dashboard background must cover the map at $point',
        );
      }
      final scrollFinder = find.descendant(
        of: find.byKey(dashboardKey),
        matching: find.byType(Scrollable),
      );
      final scrollState = tester.state<ScrollableState>(scrollFinder);
      final maximum = scrollState.position.maxScrollExtent;
      final viewport = scrollState.position.viewportDimension;
      scrollState.position.jumpTo(maximum);
      await tester.pumpAndSettle();
      final offset = scrollState.position.pixels;
      expect(offset, maximum);
      expect(offset, greaterThan(0));
      page.value = 2;
      await tester.pumpAndSettle();
      expect(scrollState.mounted, isTrue);
      expect(scrollState.position.maxScrollExtent, maximum);
      expect(scrollState.position.viewportDimension, viewport);
      expect(scrollState.position.pixels, offset);
      expect(
        await _pixel(tester, boundaryKey, const Offset(1, 300)),
        const Color(0xff2196f3),
      );
      page.value = 1;
      await tester.pumpAndSettle();
      expect(tester.state(scrollFinder), same(scrollState));
      expect(scrollState.position.pixels, offset);
      expect(
        await _pixel(tester, boundaryKey, const Offset(1, 300)),
        background,
      );
      expect(tester.state(find.byType(_PlatformMap)), same(mapState));
      expect(tester.getSize(find.byType(_PlatformMap)), const Size(400, 800));
      expect(creations, 1);
      expect(documents, 1);
    },
  );

  testWidgets(
    'Record Achievements Explorer round trip retains map and document',
    (tester) async {
      final semantics = tester.ensureSemantics();
      creations = documents = callbacks = 0;
      final tab = ValueNotifier(0);
      addTearDown(tab.dispose);
      await tester.pumpWidget(
        MaterialApp(
          home: ValueListenableBuilder<int>(
            valueListenable: tab,
            builder: (_, index, _) => AppNavigationShell(
              retainedMap: const _PlatformMap(key: ValueKey("shell map")),
              mapVisible: index == 0 || index == 2,
              navigationBuilder: (_, {required axis}) => const SizedBox(),
              bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
                  const SizedBox.expand(),
            ),
          ),
        ),
      );
      final state = tester.state(find.byType(_PlatformMap));
      for (final index in [1, 2, 1, 0]) {
        tab.value = index;
        await tester.pump();
        expect(
          tester.getSize(find.byType(_PlatformMap)).shortestSide,
          greaterThan(0),
        );
        expect(tester.state(find.byType(_PlatformMap)), same(state));
        expect(
          find.semantics.byLabel("test globe"),
          index == 1 ? findsNothing : findsOneWidget,
        );
        final before = callbacks;
        await tester.tapAt(tester.getCenter(find.byType(_PlatformMap)));
        expect(callbacks, before + (index == 1 ? 0 : 1));
      }
      expect(creations, 1);
      semantics.dispose();
      expect(documents, 1);
      expect(tester.state(find.byType(_PlatformMap)), same(state));
    },
  );
}
