import 'package:flutter/material.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';

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
      backgroundAlpha: StyleConstants.timelineGlassBackgroundAlpha,
      borderAlpha: StyleConstants.timelineGlassBorderAlpha,
      blurSigma: StyleConstants.timelineGlassBlurSigma,
      reflectionAlpha: StyleConstants.timelineGlassReflectionAlpha,
      shadowAlpha: shadowAlpha ?? StyleConstants.mapOverlayShadowAlpha,
      shadowBlurRadius: StyleConstants.mapOverlayShadowBlurRadius,
      shadowSpreadRadius: StyleConstants.mapOverlayShadowSpreadRadius,
      shadowOffset: StyleConstants.mapOverlayShadowOffset,
      padding: padding,
      child: child,
    );
  }
}
