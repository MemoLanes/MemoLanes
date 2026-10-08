import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

/// An adaptive liquid-glass surface for controls displayed over the map.
///
/// Its light and dark treatments preserve map context while keeping controls
/// readable. It is independent from the map's unexplored-area mask.
class LiquidGlassSurface extends StatelessWidget {
  const LiquidGlassSurface({
    super.key,
    required this.child,
    this.borderRadius = const BorderRadius.all(Radius.circular(16)),
    this.circular = false,
    this.backgroundAlpha,
    this.borderAlpha,
    this.blurSigma = 28,
    this.reflectionAlpha = 0.18,
    this.reflectionColor,
    this.secondaryReflectionColor,
    this.shadowAlpha,
    this.shadowBlurRadius = StyleConstants.mapOverlayShadowBlurRadius,
    this.shadowSpreadRadius = StyleConstants.mapOverlayShadowSpreadRadius,
    this.shadowOffset = StyleConstants.mapOverlayShadowOffset,
    this.padding,
  }) : assert(
         backgroundAlpha == null ||
             backgroundAlpha >= 0 && backgroundAlpha <= 1,
       ),
       assert(borderAlpha == null || borderAlpha >= 0 && borderAlpha <= 1),
       assert(blurSigma >= 0),
       assert(reflectionAlpha >= 0 && reflectionAlpha <= 1),
       assert(shadowAlpha == null || shadowAlpha >= 0 && shadowAlpha <= 1),
       assert(shadowBlurRadius >= 0);

  final Widget child;
  final BorderRadius borderRadius;
  final bool circular;
  final double? backgroundAlpha;
  final double? borderAlpha;
  final double blurSigma;
  final double reflectionAlpha;
  final Color? reflectionColor;
  final Color? secondaryReflectionColor;
  final double? shadowAlpha;
  final double shadowBlurRadius;
  final double shadowSpreadRadius;
  final Offset shadowOffset;
  final EdgeInsetsGeometry? padding;

  BoxDecoration _decoration({
    Color? color,
    Border? border,
    List<BoxShadow>? boxShadow,
  }) {
    return BoxDecoration(
      color: color,
      shape: circular ? BoxShape.circle : BoxShape.rectangle,
      borderRadius: circular ? null : borderRadius,
      border: border,
      boxShadow: boxShadow,
    );
  }

  Widget _clip(Widget child) {
    // iOS platform-view backdrop blur supports RRect clips, but ignores the
    // path clip produced by ClipOval, leaving a rectangular blur over the map.
    if (circular) {
      return ClipRRect(clipper: const _CircleRRectClipper(), child: child);
    }
    return ClipRRect(borderRadius: borderRadius, child: child);
  }

  @override
  Widget build(BuildContext context) {
    final darkProgress = context.appColors.darkProgress;
    final effectiveReflectionColor =
        reflectionColor ?? context.appColors.primaryGreen;
    final effectiveSecondaryReflectionColor =
        secondaryReflectionColor ?? context.appColors.softGreen;
    final effectiveShadowAlpha =
        shadowAlpha ?? context.appColors.mapOverlayShadowAlpha;
    final effectiveBackgroundAlpha =
        backgroundAlpha ?? lerpDouble(0.36, 0.76, darkProgress)!;
    final effectiveBorderAlpha =
        borderAlpha ?? lerpDouble(0.62, 0.46, darkProgress)!;

    return DecoratedBox(
      decoration: _decoration(
        boxShadow: [
          BoxShadow(
            color: context.appColors.shadowColor.withValues(
              alpha: effectiveShadowAlpha,
            ),
            blurRadius: shadowBlurRadius,
            spreadRadius: shadowSpreadRadius,
            offset: shadowOffset,
          ),
          BoxShadow(
            color: context.appColors.glassHighlightColor.withValues(
              alpha: lerpDouble(0.28, 0.1, darkProgress)!,
            ),
            blurRadius: 8,
            spreadRadius: -5,
            offset: const Offset(0, -2),
          ),
        ],
      ),
      child: _clip(
        BackdropFilter(
          enabled: StyleConstants.enableBackdropFilter,
          filter: ImageFilter.blur(sigmaX: blurSigma, sigmaY: blurSigma),
          child: Stack(
            fit: StackFit.passthrough,
            children: [
              Positioned.fill(
                child: DecoratedBox(
                  decoration: _decoration(
                    color: context.appColors.glassColor.withValues(
                      alpha: effectiveBackgroundAlpha,
                    ),
                    border: Border.all(
                      color: context.appColors.glassBorderColor.withValues(
                        alpha: effectiveBorderAlpha,
                      ),
                      width: 1.1,
                    ),
                  ),
                ),
              ),
              if (reflectionAlpha > 0)
                Positioned(
                  right: -18,
                  bottom: -20,
                  width: 92,
                  height: 58,
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      gradient: RadialGradient(
                        center: Alignment.bottomRight,
                        radius: 1,
                        colors: [
                          effectiveReflectionColor.withValues(
                            alpha: reflectionAlpha,
                          ),
                          effectiveSecondaryReflectionColor.withValues(
                            alpha: reflectionAlpha * 0.36,
                          ),
                          Colors.transparent,
                        ],
                        stops: const [0, 0.5, 1],
                      ),
                    ),
                  ),
                ),
              if (padding == null)
                child
              else
                Padding(padding: padding!, child: child),
            ],
          ),
        ),
      ),
    );
  }
}

class _CircleRRectClipper extends CustomClipper<RRect> {
  const _CircleRRectClipper();

  @override
  RRect getClip(Size size) {
    final radius = size.shortestSide / 2;
    return RRect.fromRectAndRadius(
      Rect.fromCircle(center: size.center(Offset.zero), radius: radius),
      Radius.circular(radius),
    );
  }

  @override
  bool shouldReclip(_CircleRRectClipper oldClipper) => false;
}
