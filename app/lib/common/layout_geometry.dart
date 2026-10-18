import 'dart:math' as math;

import 'package:flutter/painting.dart';

/// Applies insets without producing negative dimensions.
Rect insetRect(Rect rect, EdgeInsets padding) => Rect.fromLTRB(
  math.min(rect.right, rect.left + padding.left),
  math.min(rect.bottom, rect.top + padding.top),
  math.max(
    rect.left + math.min(rect.width, padding.left),
    rect.right - padding.right,
  ),
  math.max(
    rect.top + math.min(rect.height, padding.top),
    rect.bottom - padding.bottom,
  ),
);

/// Subtracts reserved regions, retaining an empty rectangle if none remain.
List<Rect> clearRegions(Rect bounds, List<Rect> reserved) {
  var result = <Rect>[bounds];
  for (final obstacle in reserved) {
    result = [for (final rect in result) ...subtractRect(rect, obstacle)];
  }
  return result.isEmpty
      ? [Rect.fromLTWH(bounds.left, bounds.top, 0, 0)]
      : result;
}

/// Returns the non-overlapping fragments outside an obstacle.
Iterable<Rect> subtractRect(Rect rect, Rect obstacle) sync* {
  if (!rect.overlaps(obstacle)) {
    yield rect;
    return;
  }
  final cut = rect.intersect(obstacle);
  final options = [
    Rect.fromLTRB(rect.left, rect.top, rect.right, cut.top),
    Rect.fromLTRB(rect.left, cut.bottom, rect.right, rect.bottom),
    Rect.fromLTRB(rect.left, cut.top, cut.left, cut.bottom),
    Rect.fromLTRB(cut.right, cut.top, rect.right, cut.bottom),
  ];
  yield* options.where((r) => r.width > 0 && r.height > 0);
}

/// Selects from a non-empty list by distance, preferring area on a tie.
Rect nearestRegion(List<Rect> regions, Offset anchor) => regions.reduce((a, b) {
  double distance(Rect r) =>
      (Offset(
                anchor.dx.clamp(r.left, r.right),
                anchor.dy.clamp(r.top, r.bottom),
              ) -
              anchor)
          .distanceSquared;
  final da = distance(a), db = distance(b);
  return db < da || (db == da && b.width * b.height > a.width * a.height)
      ? b
      : a;
});
