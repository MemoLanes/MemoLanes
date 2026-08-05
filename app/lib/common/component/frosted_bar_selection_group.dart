import 'dart:math' as math;

import 'package:flutter/material.dart';

/// Lays out equal-sized bar items with one shared, animated selection surface.
///
/// Keeping the selection surface singular lets a new selection retarget the
/// in-flight animation, instead of leaving multiple items fading out during
/// rapid taps.
class FrostedBarSelectionGroup extends StatelessWidget {
  const FrostedBarSelectionGroup({
    super.key,
    required this.selectedIndex,
    required this.children,
    this.axis = Axis.horizontal,
    this.itemExtent,
    this.crossAxisExtent,
    this.selectionInsets = const EdgeInsets.symmetric(vertical: 6),
    this.selectionAnimationDuration = const Duration(milliseconds: 250),
    this.selectionAnimationCurve = Curves.easeOutCubic,
  })  : assert(children.length > 0),
        assert(selectedIndex >= 0 && selectedIndex < children.length),
        assert(itemExtent == null || itemExtent > 0);

  final int selectedIndex;
  final List<Widget> children;
  final Axis axis;

  /// The main-axis extent of every item. When omitted, available space is
  /// divided evenly among all items.
  final double? itemExtent;

  /// The cross-axis extent, used with [itemExtent] when the group is placed
  /// in an intrinsic-size layout.
  final double? crossAxisExtent;
  final EdgeInsets selectionInsets;
  final Duration selectionAnimationDuration;
  final Curve selectionAnimationCurve;

  @override
  Widget build(BuildContext context) {
    final fixedItemExtent = itemExtent;
    final fixedCrossAxisExtent = crossAxisExtent;
    if (fixedItemExtent != null && fixedCrossAxisExtent != null) {
      return SizedBox(
        width: axis == Axis.horizontal
            ? fixedItemExtent * children.length
            : fixedCrossAxisExtent,
        height: axis == Axis.horizontal
            ? fixedCrossAxisExtent
            : fixedItemExtent * children.length,
        child: _buildContent(
          itemExtent: fixedItemExtent,
          crossAxisExtent: fixedCrossAxisExtent,
        ),
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final availableMainAxisExtent = axis == Axis.horizontal
            ? constraints.maxWidth
            : constraints.maxHeight;
        final currentItemExtent =
            fixedItemExtent ?? availableMainAxisExtent / children.length;
        final availableCrossAxisExtent = axis == Axis.horizontal
            ? constraints.maxHeight
            : constraints.maxWidth;
        return _buildContent(
          itemExtent: currentItemExtent,
          crossAxisExtent: fixedCrossAxisExtent ?? availableCrossAxisExtent,
        );
      },
    );
  }

  Widget _buildContent({
    required double itemExtent,
    required double crossAxisExtent,
  }) {
    final selectionWidth = axis == Axis.horizontal
        ? itemExtent - selectionInsets.horizontal
        : crossAxisExtent - selectionInsets.horizontal;
    final selectionHeight = axis == Axis.horizontal
        ? crossAxisExtent - selectionInsets.vertical
        : itemExtent - selectionInsets.vertical;

    return Stack(
      fit: StackFit.expand,
      children: [
        AnimatedPositioned(
          duration: selectionAnimationDuration,
          curve: selectionAnimationCurve,
          left: axis == Axis.horizontal
              ? itemExtent * selectedIndex + selectionInsets.left
              : selectionInsets.left,
          top: axis == Axis.horizontal
              ? selectionInsets.top
              : itemExtent * selectedIndex + selectionInsets.top,
          width: math.max(0, selectionWidth),
          height: math.max(0, selectionHeight),
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: 0.5),
              borderRadius: BorderRadius.circular(8),
            ),
          ),
        ),
        axis == Axis.horizontal
            ? Row(
                children: [
                  for (final child in children) Expanded(child: child),
                ],
              )
            : Column(
                children: [
                  for (final child in children) Expanded(child: child),
                ],
              ),
      ],
    );
  }
}
