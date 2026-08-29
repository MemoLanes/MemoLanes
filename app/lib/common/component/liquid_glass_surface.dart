import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';

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
  })  : assert(backgroundAlpha == null ||
            backgroundAlpha >= 0 && backgroundAlpha <= 1),
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
    if (circular) return ClipOval(child: child);
    return ClipRRect(borderRadius: borderRadius, child: child);
  }

  @override
  Widget build(BuildContext context) {
    final effectiveReflectionColor =
        reflectionColor ?? StyleConstants.primaryGreen;
    final effectiveSecondaryReflectionColor =
        secondaryReflectionColor ?? StyleConstants.softGreen;
    final effectiveShadowAlpha =
        shadowAlpha ?? StyleConstants.mapOverlayShadowAlpha;
    final effectiveBackgroundAlpha =
        backgroundAlpha ?? (StyleConstants.isDarkMode ? 0.76 : 0.36);
    final effectiveBorderAlpha =
        borderAlpha ?? (StyleConstants.isDarkMode ? 0.46 : 0.62);

    return DecoratedBox(
      decoration: _decoration(
        boxShadow: [
          BoxShadow(
            color: StyleConstants.shadowColor.withValues(
              alpha: effectiveShadowAlpha,
            ),
            blurRadius: shadowBlurRadius,
            spreadRadius: shadowSpreadRadius,
            offset: shadowOffset,
          ),
          BoxShadow(
            color: StyleConstants.glassHighlightColor.withValues(
              alpha: StyleConstants.isDarkMode ? 0.1 : 0.28,
            ),
            blurRadius: 8,
            spreadRadius: -5,
            offset: const Offset(0, -2),
          ),
        ],
      ),
      child: _clip(
        BackdropFilter(
          filter: ImageFilter.blur(sigmaX: blurSigma, sigmaY: blurSigma),
          child: Stack(
            fit: StackFit.passthrough,
            children: [
              Positioned.fill(
                child: DecoratedBox(
                  decoration: _decoration(
                    color: StyleConstants.glassColor
                        .withValues(alpha: effectiveBackgroundAlpha),
                    border: Border.all(
                      color: StyleConstants.glassBorderColor
                          .withValues(alpha: effectiveBorderAlpha),
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
              Positioned(
                left: circular ? 10 : 14,
                right: circular ? 10 : 14,
                top: 1,
                height: 1.2,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    gradient: LinearGradient(
                      colors: [
                        StyleConstants.glassHighlightColor.withValues(alpha: 0),
                        StyleConstants.glassHighlightColor.withValues(
                          alpha: StyleConstants.isDarkMode ? 0.32 : 0.88,
                        ),
                        StyleConstants.glassHighlightColor.withValues(alpha: 0),
                      ],
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
