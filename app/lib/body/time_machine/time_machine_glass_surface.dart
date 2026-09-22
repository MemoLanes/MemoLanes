import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

/// Shared glass treatment for controls in the time-machine overlay.
class TimeMachineGlassSurface extends StatelessWidget {
  const TimeMachineGlassSurface({
    super.key,
    required this.child,
    this.borderRadius = const BorderRadius.all(Radius.circular(12)),
    this.padding,
    this.shadowAlpha,
  }) : assert(shadowAlpha == null || shadowAlpha >= 0 && shadowAlpha <= 1);

  final Widget child;
  final BorderRadius borderRadius;
  final EdgeInsetsGeometry? padding;
  final double? shadowAlpha;

  @override
  Widget build(BuildContext context) {
    return LiquidGlassSurface(
      borderRadius: borderRadius,
      backgroundAlpha: lerpDouble(0.60, 0.84, context.appColors.darkProgress)!,
      borderAlpha: lerpDouble(0.84, 0.46, context.appColors.darkProgress)!,
      blurSigma: StyleConstants.timelineGlassBlurSigma,
      reflectionAlpha: 0.12,
      shadowAlpha: shadowAlpha ?? context.appColors.mapOverlayShadowAlpha,
      shadowBlurRadius: StyleConstants.mapOverlayShadowBlurRadius,
      shadowSpreadRadius: StyleConstants.mapOverlayShadowSpreadRadius,
      shadowOffset: StyleConstants.mapOverlayShadowOffset,
      padding: padding,
      child: child,
    );
  }
}
