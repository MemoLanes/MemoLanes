import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/duo_hardware_test_data.dart';

import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/app_layout.dart';

const _duo = DuoHardware(
  supported: true,
  hasHinge: true,
  status: DuoHingeStatus.fullyOpen,
  angle: 180,
  folds: [],
);

AppLayout layout({
  Size size = const Size(951, 669),
  Size? bodySize,
  EdgeInsets padding = EdgeInsets.zero,
  EdgeInsets keyboard = EdgeInsets.zero,
  DuoHardware fold = _duo,
  DuoBarEdge edge = DuoBarEdge.right,
  TargetPlatform platform = TargetPlatform.iOS,
  bool navigation = true,
}) => AppLayout.resolve(
  media: MediaQueryData(size: size, viewPadding: padding, viewInsets: keyboard),
  size: bodySize ?? size,
  hardware: fold,
  barEdge: edge,
  platform: platform,
  navigationVisible: navigation,
);

void contained(Rect child, Rect bounds) {
  expect(child.left, greaterThanOrEqualTo(bounds.left));
  expect(child.top, greaterThanOrEqualTo(bounds.top));
  expect(child.right, lessThanOrEqualTo(bounds.right));
  expect(child.bottom, lessThanOrEqualTo(bounds.bottom));
}

void main() {
  test('wider clear regions never make a card narrower', () {
    for (final hardware in [DuoHardware.none, _duo]) {
      var previous = 0.0;
      var previousPicker = 0.0;
      for (var width = 260.0; width <= 1100; width++) {
        final resolved = layout(
          size: Size(width, 466),
          navigation: false,
          padding: const EdgeInsets.only(right: 84, bottom: 34),
          fold: hardware,
        );
        final card = resolved.panelBounds;
        contained(card, resolved.safeBounds);
        expect(
          card.width,
          greaterThanOrEqualTo(previous),
          reason: 'width: $width',
        );
        previous = card.width;
        final picker = resolved.pickerBounds;
        contained(picker, resolved.safeBounds);
        expect(picker.width, greaterThanOrEqualTo(previousPicker));
        previousPicker = picker.width;
      }
    }
  });

  test(
    'browsing cards leave map space and retain portrait width with a keyboard',
    () {
      for (final hardware in [DuoHardware.none, _duo]) {
        final portrait = layout(
          size: const Size(669, 951),
          padding: const EdgeInsets.fromLTRB(0, 24, 0, 34),
          edge: DuoBarEdge.horizontal,
          fold: hardware,
        );
        final keyboard = layout(
          size: const Size(669, 951),
          bodySize: const Size(669, 651),
          keyboard: const EdgeInsets.only(bottom: 300),
          padding: const EdgeInsets.fromLTRB(0, 24, 0, 34),
          edge: DuoBarEdge.horizontal,
          fold: hardware,
        );
        contained(portrait.pickerBounds, portrait.safeBounds);
        contained(keyboard.pickerBounds, keyboard.safeBounds);
        final fixedViewport = layout(
          size: const Size(669, 951),
          keyboard: const EdgeInsets.only(bottom: 300),
          padding: const EdgeInsets.fromLTRB(0, 24, 0, 34),
          edge: DuoBarEdge.horizontal,
          fold: hardware,
        );
        expect(fixedViewport.hasSameMapViewport(portrait), isTrue);
        expect(fixedViewport.safeBounds.bottom, keyboard.safeBounds.bottom);
        expect(fixedViewport.detailBounds.bottom, keyboard.detailBounds.bottom);
        contained(fixedViewport.pickerBounds, fixedViewport.safeBounds);
        expect(fixedViewport.hasSameMapViewport(keyboard), isFalse);
        expect(
          portrait.pickerBounds.width,
          lessThan(portrait.safeBounds.width),
        );
        expect(
          portrait.pickerBounds.height,
          lessThan(portrait.safeBounds.height),
        );
        expect(keyboard.pickerBounds.width, portrait.pickerBounds.width);
        expect(
          keyboard.pickerBounds.height,
          lessThanOrEqualTo(portrait.pickerBounds.height),
        );
      }
    },
  );

  test(
    'ordinary panel fitting leaves a positive viewport after the keyboard',
    () {
      for (final size in [
        const Size(393, 200),
        const Size(300, 120),
        const Size(80, 80),
      ]) {
        final ordinary = AppLayout.resolve(
          media: const MediaQueryData(
            size: Size(393, 852),
            padding: EdgeInsets.only(top: 59),
            viewInsets: EdgeInsets.only(bottom: 500),
          ),
          size: size,
          hardware: DuoHardware.none,
          barEdge: DuoBarEdge.unknown,
          platform: TargetPlatform.iOS,
        );
        final padding = ordinary.mapFitPaddingForOverlay(
          Rect.fromLTWH(0, 0, size.width, size.height),
        );
        expect(padding.horizontal, lessThan(size.width));
        expect(padding.vertical, lessThan(size.height));
      }
    },
  );

  test('a fold alone does not locate an unknown camera in the side strip', () {
    const fold = DuoRegion(Rect.fromLTWH(0, 190, 600, 40), isActive: true);
    final duo = layout(
      size: const Size(600, 420),
      padding: const EdgeInsets.only(right: 84),
      fold: _duo.copyWith(status: DuoHingeStatus.partiallyOpen, folds: [fold]),
    );
    expect(duo.navigationBounds.right, lessThanOrEqualTo(duo.safeBounds.right));
  });
  test(
    'fit padding always leaves a positive area in short or keyboard views',
    () {
      for (final size in [
        const Size(600, 240),
        const Size(512, 240),
        const Size(393, 200),
        const Size(300, 120),
      ]) {
        final duo = layout(size: size);
        for (final padding in [
          duo.mapFitPadding,
          duo.fitPaddingFor(Rect.fromLTWH(0, size.height - 40, size.width, 20)),
        ]) {
          for (final edge in [
            padding.left,
            padding.top,
            padding.right,
            padding.bottom,
          ]) {
            expect(edge.isFinite, isTrue);
            expect(edge, greaterThanOrEqualTo(0));
          }
          expect(padding.horizontal, lessThan(size.width));
          expect(padding.vertical, lessThan(size.height));
        }
      }
    },
  );
  test('hardware capability, platform and native edge are all required', () {
    for (final size in [
      const Size(393, 852),
      const Size(844, 390),
      const Size(1024, 768),
    ]) {
      final ordinary = layout(
        size: size,
        fold: const DuoHardware(
          supported: true,
          horizontalSizeClass: DuoSizeClass.regular,
          folds: [DuoRegion(Rect.fromLTWH(400, 0, 20, 768))],
        ),
      );
      expect(ordinary.detachedBackBounds, isNull);
      expect(ordinary.contentBounds, Offset.zero & size);
    }
    expect(layout(platform: TargetPlatform.android).detachedBackBounds, isNull);
    expect(layout(edge: DuoBarEdge.unknown).detachedBackBounds, isNull);
    // Empty display features cannot infer either hardware or current posture.
    expect(layout(fold: DuoHardware.none).detachedBackBounds, isNull);
    expect(layout(fold: _duo).detachedBackBounds, isNotNull);
  });

  test(
    'physical edges follow the native reading regardless of aspect ratio',
    () {
      final cover = layout(size: const Size(420, 922));
      expect(cover.navigationAxis, Axis.vertical);
      expect(
        cover.navigationBounds.center.dx,
        greaterThan(cover.contentBounds.center.dx),
      );
      final leftPane = layout(
        size: const Size(460, 669),
        edge: DuoBarEdge.left,
      );
      expect(
        leftPane.navigationBounds.right,
        lessThan(leftPane.contentBounds.left),
      );
      final portrait = layout(
        size: const Size(669, 951),
        edge: DuoBarEdge.horizontal,
        padding: const EdgeInsets.only(top: 62, bottom: 34),
      );
      expect(portrait.navigationAxis, Axis.horizontal);
      contained(portrait.navigationBounds, portrait.safeBounds);
      expect(
        portrait.navigationBounds.top,
        greaterThan(portrait.contentBounds.bottom),
      );
    },
  );

  test('map tools stay opposite either physical vertical navigation edge', () {
    for (final size in [
      const Size(393, 852),
      const Size(852, 393),
      const Size(852, 240),
    ]) {
      for (final edge in [DuoBarEdge.left, DuoBarEdge.right]) {
        final duo = layout(
          size: size,
          edge: edge,
          padding: EdgeInsets.fromLTRB(
            edge == DuoBarEdge.left ? 96 : 8,
            24,
            edge == DuoBarEdge.right ? 96 : 8,
            24,
          ),
        );
        final tools = duo.mapControlsBounds;
        contained(tools, duo.safeBounds);
        expect(tools.overlaps(duo.navigationBounds), isFalse);
        if (edge == DuoBarEdge.right) {
          expect(tools.right, lessThan(duo.navigationBounds.left));
        } else {
          expect(tools.left, greaterThan(duo.navigationBounds.right));
        }
        expect(
          tools.size,
          duo.mapControlsAxis == Axis.vertical
              ? const Size(44, 148)
              : const Size(148, 44),
        );
      }
    }
  });

  test('opposite map tools select a pane clear of the actual book fold', () {
    const fold = DuoRegion(Rect.fromLTWH(413, 0, 40, 669), isActive: true);
    for (final edge in [DuoBarEdge.left, DuoBarEdge.right]) {
      final duo = layout(
        edge: edge,
        padding: const EdgeInsets.symmetric(horizontal: 9),
        fold: _duo.copyWith(
          status: DuoHingeStatus.partiallyOpen,
          folds: [fold],
        ),
      );
      final tools = duo.mapControlsBounds;
      expect(tools.overlaps(fold.frame), isFalse);
      expect(tools.overlaps(duo.navigationBounds), isFalse);
      contained(tools, duo.safeBounds);
      if (edge == DuoBarEdge.right) {
        expect(tools.right, lessThanOrEqualTo(fold.frame.left));
        // The main recording controls retain their original right-hand pane.
        expect(duo.controlRegion.left, greaterThanOrEqualTo(fold.frame.right));
      } else {
        expect(tools.left, greaterThanOrEqualTo(fold.frame.right));
      }
    }
  });

  test(
    'top Back and bottom tabs share the native column even with tabs hidden',
    () {
      const size = Size(951, 669);
      const stripWidth = 108.0;
      for (final edge in [DuoBarEdge.left, DuoBarEdge.right]) {
        Rect occlusion(
          double offset,
          double top,
          double width,
          double height,
        ) => Rect.fromLTWH(
          edge == DuoBarEdge.right
              ? size.width - stripWidth + offset
              : stripWidth - offset - width,
          top,
          width,
          height,
        );
        final camera = occlusion(32, 0, 64, 170);
        final status = occlusion(21, 190, 78, 60);
        final fold = _duo.copyWith(
          cameras: [
            for (final bounds in [camera, status])
              DuoRegion(bounds, isActive: true),
          ],
        );
        final padding = EdgeInsets.fromLTRB(
          edge == DuoBarEdge.left ? stripWidth : 9,
          12,
          edge == DuoBarEdge.right ? stripWidth : 9,
          12,
        );
        final root = layout(
          size: size,
          edge: edge,
          padding: padding,
          fold: fold,
        );
        final details = layout(
          size: size,
          edge: edge,
          padding: padding,
          fold: fold,
          navigation: false,
        );
        final strip = Rect.fromLTWH(
          edge == DuoBarEdge.left ? 0 : size.width - stripWidth,
          12,
          stripWidth,
          size.height - 24,
        );
        final back = details.primaryNavigationBounds;
        expect(details.navigationBounds, Rect.zero);
        expect(back, root.primaryNavigationBounds);
        expect(back.size, const Size(44, 44));
        expect(back.center.dx, root.navigationBounds.center.dx);
        expect(back.top, status.bottom + 14);
        expect(back.bottom, lessThan(root.navigationBounds.top));
        contained(back, strip);
        contained(root.navigationBounds, strip);
        for (final bounds in [camera, status]) {
          expect(back.overlaps(bounds), isFalse);
          expect(root.navigationBounds.overlaps(bounds), isFalse);
        }
      }
    },
  );

  test(
    'Back falls back to native safe edges without guessing a hardware strip',
    () {
      for (final edge in [DuoBarEdge.left, DuoBarEdge.right]) {
        final root = layout(
          edge: edge,
          padding: const EdgeInsets.fromLTRB(9, 28, 108, 12),
        );
        final details = layout(
          edge: edge,
          padding: const EdgeInsets.fromLTRB(9, 28, 108, 12),
          navigation: false,
        );
        final back = details.primaryNavigationBounds;
        expect(back.size, const Size(44, 44));
        expect(back, root.primaryNavigationBounds);
        expect(back.center.dx, root.navigationBounds.center.dx);
        contained(back, details.safeBounds);
      }
    },
  );

  test(
    'no vertical bar retains a leading top Back clear of reported occlusions',
    () {
      const occlusion = DuoRegion(Rect.fromLTWH(0, 0, 669, 80), isActive: true);
      for (final navigation in [false, true]) {
        final duo = layout(
          size: const Size(669, 951),
          edge: DuoBarEdge.horizontal,
          padding: const EdgeInsets.fromLTRB(9, 62, 9, 34),
          fold: _duo.copyWith(cameras: [occlusion]),
          navigation: navigation,
        );
        final back = duo.primaryNavigationBounds;
        expect(back.size, const Size(44, 44));
        expect(back.top, occlusion.frame.bottom + 14);
        expect(back.overlaps(occlusion.frame), isFalse);
        contained(back, duo.safeBounds);
      }
    },
  );

  test('full and book cards use the available pane on either native edge', () {
    const size = Size(951, 669);
    const division = DuoRegion(
      Rect.fromLTRB(455.5, 0, 495.5, 669),
      isActive: true,
    );
    for (final edge in [DuoBarEdge.left, DuoBarEdge.right]) {
      final camera = DuoRegion(
        Rect.fromLTWH(edge == DuoBarEdge.right ? 867 : 0, 0, 84, 120),
        isActive: true,
      );
      final padding = EdgeInsets.fromLTRB(
        edge == DuoBarEdge.left ? 84 : 0,
        0,
        edge == DuoBarEdge.right ? 84 : 0,
        34,
      );
      for (final navigation in [false, true]) {
        final full = layout(
          size: size,
          edge: edge,
          padding: padding,
          fold: _duo.copyWith(cameras: [camera]),
          navigation: navigation,
        );
        final book = layout(
          size: size,
          edge: edge,
          padding: padding,
          fold: _duo.copyWith(
            status: DuoHingeStatus.partiallyOpen,
            folds: [division],
            cameras: [camera],
          ),
          navigation: navigation,
        );
        expect(full.panelBounds.width, greaterThan(book.panelBounds.width));
        expect(book.panelBounds.width, greaterThan(300));
        contained(full.panelBounds, full.safeBounds);
        contained(book.panelBounds, book.safeBounds);
        expect(book.panelBounds.overlaps(division.frame), isFalse);
        expect(book.panelBounds.overlaps(camera.frame), isFalse);
      }
    }
  });

  test(
    'the hardware strip uses actual camera bounds and independent safe edges',
    () {
      const camera = DuoRegion(Rect.fromLTWH(867, 0, 84, 172), isActive: true);
      final duo = layout(
        padding: const EdgeInsets.only(left: 9, right: 84),
        fold: _duo.copyWith(cameras: [camera]),
      );
      expect(duo.safeBounds.left, 9);
      expect(duo.safeBounds.right, 867);
      expect(duo.navigationBounds.left, greaterThanOrEqualTo(867));
      expect(duo.navigationBounds.overlaps(camera.frame), isFalse);
      contained(duo.panelBounds, duo.safeBounds);
      contained(duo.mapControlsBounds, duo.controlRegion);
    },
  );

  test(
    'book fold uses its reported coordinates even while activation lags',
    () {
      const fold = DuoRegion(Rect.fromLTWH(413, 0, 40, 669), isActive: false);
      final duo = layout(
        fold: _duo.copyWith(
          status: DuoHingeStatus.partiallyOpen,
          folds: [fold],
        ),
      );
      expect(duo.panelBounds.right, lessThanOrEqualTo(fold.frame.left));
      expect(duo.controlRegion.left, greaterThanOrEqualTo(fold.frame.right));
      expect(duo.mapControlsBounds.overlaps(fold.frame), isFalse);
      expect(duo.mapFitPadding.left, greaterThan(duo.panelBounds.right));
      final flat = layout(fold: _duo.copyWith(folds: [fold.copyActive()]));
      expect(flat.regions, hasLength(1));
    },
  );

  test('tabletop controls, panel and popup stay below the actual fold', () {
    const fold = DuoRegion(Rect.fromLTWH(0, 455, 669, 40), isActive: false);
    final duo = layout(
      size: const Size(669, 951),
      edge: DuoBarEdge.horizontal,
      padding: const EdgeInsets.only(top: 62, bottom: 34),
      fold: _duo.copyWith(status: DuoHingeStatus.partiallyOpen, folds: [fold]),
    );
    expect(duo.panelBounds.top, greaterThanOrEqualTo(495));
    expect(duo.controlRegion.top, greaterThanOrEqualTo(495));
    expect(
      duo.regionFor(duo.panelBounds.center).top,
      greaterThanOrEqualTo(495),
    );
    expect(duo.mapFitPadding.bottom, greaterThan(951 - fold.frame.top));
  });

  test(
    'keyboard uses body constraints once and chooses a region with room',
    () {
      const fold = DuoRegion(Rect.fromLTWH(0, 455, 669, 40), isActive: true);
      final duo = layout(
        size: const Size(669, 951),
        bodySize: const Size(669, 571),
        edge: DuoBarEdge.horizontal,
        navigation: false,
        padding: const EdgeInsets.only(top: 62, bottom: 34),
        keyboard: const EdgeInsets.only(bottom: 380),
        fold: _duo.copyWith(
          status: DuoHingeStatus.partiallyOpen,
          folds: [fold],
        ),
      );
      expect(duo.size.height, 571);
      expect(duo.safeBounds.bottom, 571);
      contained(duo.panelBounds, duo.controlRegion);
      contained(duo.mapControlsBounds, duo.controlRegion);
      expect(duo.controlRegion.bottom, lessThanOrEqualTo(fold.frame.top));
    },
  );

  test('controls choose a region for their current natural height', () {
    const fold = DuoRegion(Rect.fromLTWH(0, 382, 852, 28), isActive: true);
    final duo = layout(
      size: const Size(852, 793),
      bodySize: const Size(852, 566),
      navigation: false,
      padding: const EdgeInsets.all(12),
      keyboard: const EdgeInsets.only(bottom: 227),
      fold: _duo.copyWith(status: DuoHingeStatus.partiallyOpen, folds: [fold]),
    );
    final closed = duo.controlRegionFor(minimumHeight: 88);
    final opened = duo.controlRegionFor(minimumHeight: 191);
    expect(closed.top, fold.frame.bottom);
    expect(opened.bottom, fold.frame.top);
    expect(opened.height, greaterThanOrEqualTo(191));
    expect(opened.overlaps(fold.frame), isFalse);
    // A control taller than every region gets the largest safe scroll viewport.
    final tooTall = duo.controlRegionFor(minimumHeight: 500);
    expect(tooTall, opened);
    contained(tooTall, duo.safeBounds);
  });

  test(
    'stale off-window bounds and inactive flat folds do not divide the UI',
    () {
      const oldFold = DuoRegion(Rect.fromLTWH(413, 0, 40, 951), isActive: true);
      final duo = layout(
        fold: _duo.copyWith(
          status: DuoHingeStatus.partiallyOpen,
          folds: [oldFold],
        ),
      );
      expect(duo.regions, hasLength(1));
    },
  );
}

extension on DuoRegion {
  DuoRegion copyActive() => DuoRegion(frame, margins: margins, isActive: true);
}
