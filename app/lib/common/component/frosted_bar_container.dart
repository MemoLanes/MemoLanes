import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';

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
    this.backgroundAlpha,
  });

  final Widget child;
  final Axis axis;
  final double extent;
  final double mainAxisPadding;
  final double crossAxisPadding;
  final double radius;
  final double blurSigma;
  final double? backgroundAlpha;

  @override
  Widget build(BuildContext context) {
    final effectiveBackgroundAlpha =
        backgroundAlpha ?? (StyleConstants.isDarkMode ? 0.86 : 0.7);

    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: BackdropFilter(
        enabled: StyleConstants.enableBackdropFilter,
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
            color: StyleConstants.glassColor.withValues(
              alpha: effectiveBackgroundAlpha,
            ),
            borderRadius: BorderRadius.circular(radius),
            border: Border.all(
              color: StyleConstants.glassBorderColor.withValues(
                alpha: StyleConstants.isDarkMode ? 0.42 : 0.4,
              ),
            ),
            boxShadow: [
              BoxShadow(
                color: StyleConstants.shadowColor.withValues(
                  alpha: StyleConstants.isDarkMode ? 0.42 : 0.08,
                ),
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
