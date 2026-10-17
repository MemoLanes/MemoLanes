import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/duo_device.dart';

import 'support/duo_hardware_test_data.dart';

void main() {
  testWidgets(
    'cards stay centered and map fitting avoids them through resizing and folds',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final panel = GlobalKey<MapOverlayPanelState>();
      for (final (size, edge, fold, keyboard, height) in [
        (const Size(393, 852), DuoBarEdge.unknown, null, 0.0, 123.0),
        (
          const Size(951, 669),
          DuoBarEdge.right,
          const Rect.fromLTWH(460, 0, 20, 669),
          0.0,
          450.0,
        ),
        (
          const Size(669, 951),
          DuoBarEdge.horizontal,
          const Rect.fromLTWH(0, 475, 669, 20),
          380.0,
          200.0,
        ),
        (const Size(678, 466), DuoBarEdge.left, null, 0.0, 310.0),
      ]) {
        tester.view.physicalSize = size;
        final padding = EdgeInsets.only(
          left: edge == DuoBarEdge.left ? 84 : 0,
          right: edge == DuoBarEdge.right ? 84 : 0,
          bottom: 34,
        );
        final media = MediaQueryData(
          size: size,
          padding: padding.copyWith(bottom: keyboard > 0 ? 0 : 34),
          viewPadding: padding,
          viewInsets: EdgeInsets.only(bottom: keyboard),
        );
        final layout = AppLayout.resolve(
          media: media,
          size: size,
          hardware: edge == DuoBarEdge.unknown
              ? DuoHardware.none
              : DuoHardware(
                  hasHinge: true,
                  status: fold == null
                      ? DuoHingeStatus.fullyOpen
                      : DuoHingeStatus.partiallyOpen,
                  folds: [if (fold != null) DuoRegion(fold, isActive: true)],
                  cameras: [
                    if (edge == DuoBarEdge.left || edge == DuoBarEdge.right)
                      DuoRegion(
                        Rect.fromLTWH(
                          edge == DuoBarEdge.left ? 0 : size.width - 84,
                          0,
                          84,
                          120,
                        ),
                        isActive: true,
                      ),
                  ],
                ),
          barEdge: edge,
          platform: TargetPlatform.iOS,
          navigationVisible: false,
        );
        await tester.pumpWidget(
          MaterialApp(
            home: MediaQuery(
              data: media,
              child: Scaffold(
                resizeToAvoidBottomInset: false,
                body: AppLayoutScope(
                  layout: layout,
                  child: Stack(
                    children: [
                      MapOverlayPanel.detail(
                        key: panel,
                        child: PageContentInsets(
                          maintainViewPadding: true,
                          child: SizedBox(
                            key: const Key('card'),
                            height: height,
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
        final occupied = tester.getRect(find.byKey(const Key('card')));
        expect(
          MediaQuery.paddingOf(tester.element(find.byKey(const Key('card')))),
          EdgeInsets.zero,
        );
        expect(
          occupied.center.dx,
          closeTo(layout.regionFor(occupied.center).center.dx, 0.01),
        );
        final visible = panel.currentState!.mapFitPadding.deflateRect(
          Offset.zero & size,
        );
        expect(occupied.height, lessThanOrEqualTo(height));
        expect(occupied.overlaps(layout.primaryNavigationBounds), isFalse);
        expect(visible.width, greaterThan(0));
        expect(visible.height, greaterThan(0));
        expect(visible.overlaps(occupied), isFalse);
        if (fold != null) expect(visible.overlaps(fold), isFalse);
        expect(tester.takeException(), isNull);
      }
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );
}
