import 'dart:ui';

import 'package:flutter/material.dart';

class FrostedBarContainer extends StatelessWidget {
  const FrostedBarContainer({
    super.key,
    required this.child,
    this.axis = Axis.horizontal,
    this.extent = 64,
    this.mainAxisPadding = 8,
    this.crossAxisPadding = 0,
    this.radius = 16,
    this.blurSigma = 12,
    this.backgroundAlpha = 0.7,
  });

  final Widget child;
  final Axis axis;
  final double extent;
  final double mainAxisPadding;
  final double crossAxisPadding;
  final double radius;
  final double blurSigma;
  final double backgroundAlpha;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: blurSigma, sigmaY: blurSigma),
        child: Container(
          width: axis == Axis.vertical ? extent : null,
          height: axis == Axis.horizontal ? extent : null,
          padding: axis == Axis.horizontal
              ? EdgeInsets.symmetric(
                  horizontal: mainAxisPadding,
                  vertical: crossAxisPadding,
                )
              : EdgeInsets.symmetric(
                  horizontal: crossAxisPadding,
                  vertical: mainAxisPadding,
                ),
          decoration: BoxDecoration(
            color: Colors.white.withValues(alpha: backgroundAlpha),
            borderRadius: BorderRadius.circular(radius),
            border: Border.all(color: Colors.white.withValues(alpha: 0.4)),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.08),
                blurRadius: 20,
                offset: const Offset(0, 4),
              ),
            ],
          ),
          child: child,
        ),
      ),
    );
  }
}
