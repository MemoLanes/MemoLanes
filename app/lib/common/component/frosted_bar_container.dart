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
    this.blurSigma = 2,
    this.backgroundAlpha = 0.8,
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
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.1),
                blurRadius: 8,
                offset: const Offset(0, 2),
              ),
            ],
          ),
          child: child,
        ),
      ),
    );
  }
}
