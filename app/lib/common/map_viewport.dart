import 'dart:math' as math;

import 'package:flutter/painting.dart';
import 'package:memolanes/common/layout_geometry.dart' as geometry;
import 'package:memolanes/constants/style_constants.dart';

/// Fits a map in the remaining clear area around an overlay's actual footprint.
EdgeInsets fitPaddingAroundOverlay({
  required Size viewportSize,
  required List<Rect> regions,
  required Rect occupied,
}) {
  final clear = [
    for (final region in regions)
      ...geometry.subtractRect(region, occupied.inflate(16)),
  ];
  final fitting = clear.where(
    (r) =>
        r.width >= StyleConstants.minimumMapExtent &&
        r.height >= StyleConstants.minimumMapExtent,
  );
  // Prefer a spacious viewport, but keep a short remaining area ahead of
  // fitting behind the card when the keyboard or a small window leaves less.
  var visible = (fitting.isNotEmpty ? fitting : clear).fold(
    Rect.zero,
    (a, b) => b.width * b.height > a.width * a.height ? b : a,
  );
  // Only a completely covered window needs the full-region fallback.
  if (visible.isEmpty) {
    visible = regions.reduce(
      (a, b) => a.width * a.height > b.width * b.height ? a : b,
    );
  }
  return fitPaddingForRegion(viewportSize, visible);
}

/// Keeps MapLibre's fit area positive even when a short window is covered.
EdgeInsets fitPaddingForRegion(Size viewportSize, Rect visible) {
  final viewport = Offset.zero & viewportSize;
  final intersection = visible.intersect(viewport);
  final area = intersection.isEmpty ? viewport : intersection;
  final horizontalGap = math.max(0.0, (area.width - 1) / 2);
  final verticalGap = math.max(0.0, (area.height - 1) / 2);
  // Shrink margins inside the chosen area; never enlarge it across a card
  // or hinge just to keep a minimum camera viewport.
  final fitted = geometry.insetRect(
    area,
    EdgeInsets.fromLTRB(
      math.min(horizontalGap, math.max(16.0, 28 - area.left)),
      math.min(verticalGap, math.max(16.0, 28 - area.top)),
      math.min(
        horizontalGap,
        math.max(16.0, 28 - (viewportSize.width - area.right)),
      ),
      math.min(
        verticalGap,
        math.max(16.0, 28 - (viewportSize.height - area.bottom)),
      ),
    ),
  );
  return EdgeInsets.fromLTRB(
    fitted.left,
    fitted.top,
    viewportSize.width - fitted.right,
    viewportSize.height - fitted.bottom,
  );
}
