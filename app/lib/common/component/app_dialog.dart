import 'package:flutter/material.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

enum AppDialogSurfaceStyle { solid, glass }

class AppDialogSurface extends StatelessWidget {
  const AppDialogSurface({
    super.key,
    required this.child,
    this.style = AppDialogSurfaceStyle.solid,
    this.borderRadius = const BorderRadius.all(Radius.circular(24)),
    this.backgroundColor,
    this.glassBackgroundAlpha = 0.84,
    this.shadowAlpha = 0.18,
    this.shadowBlurRadius = 32,
    this.shadowSpreadRadius = -8,
    this.shadowOffset = const Offset(0, 12),
  }) : assert(glassBackgroundAlpha >= 0 && glassBackgroundAlpha <= 1),
       assert(shadowAlpha >= 0 && shadowAlpha <= 1),
       assert(shadowBlurRadius >= 0);

  final Widget child;
  final AppDialogSurfaceStyle style;
  final BorderRadius borderRadius;
  final Color? backgroundColor;
  final double glassBackgroundAlpha;
  final double shadowAlpha;
  final double shadowBlurRadius;
  final double shadowSpreadRadius;
  final Offset shadowOffset;

  @override
  Widget build(BuildContext context) {
    if (style == AppDialogSurfaceStyle.glass) {
      return LiquidGlassSurface(
        borderRadius: borderRadius,
        backgroundAlpha: glassBackgroundAlpha,
        borderAlpha: 0.72,
        blurSigma: 32,
        reflectionAlpha: 0.1,
        shadowAlpha: shadowAlpha,
        shadowBlurRadius: shadowBlurRadius,
        shadowSpreadRadius: shadowSpreadRadius,
        shadowOffset: shadowOffset,
        child: Material(color: Colors.transparent, child: child),
      );
    }

    return DecoratedBox(
      decoration: BoxDecoration(
        color: backgroundColor ?? StyleConstants.canvasColor,
        borderRadius: borderRadius,
        border: Border.all(color: StyleConstants.lineColor),
        boxShadow: [
          BoxShadow(
            color: StyleConstants.shadowColor.withValues(alpha: shadowAlpha),
            blurRadius: shadowBlurRadius,
            spreadRadius: shadowSpreadRadius,
            offset: shadowOffset,
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: borderRadius,
        child: Material(color: Colors.transparent, child: child),
      ),
    );
  }
}

class AppDialogCard extends StatelessWidget {
  const AppDialogCard({
    super.key,
    required this.child,
    this.title,
    this.subtitle,
    this.leading,
    this.actions,
    this.showHeader = true,
    this.surfaceStyle = AppDialogSurfaceStyle.solid,
    this.maxHeightFactor = 0.78,
    this.contentPadding = const EdgeInsets.fromLTRB(20, 12, 20, 18),
    this.backgroundColor,
  }) : assert(maxHeightFactor > 0 && maxHeightFactor <= 1);

  final Widget child;
  final String? title;
  final String? subtitle;
  final Widget? leading;
  final Widget? actions;
  final bool showHeader;
  final AppDialogSurfaceStyle surfaceStyle;
  final double maxHeightFactor;
  final EdgeInsetsGeometry contentPadding;
  final Color? backgroundColor;

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.sizeOf(context).height * maxHeightFactor,
      ),
      child: AppDialogSurface(
        style: surfaceStyle,
        backgroundColor: backgroundColor,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (showHeader && title != null) ...[
              const SizedBox(height: 14),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                child: Row(
                  children: [
                    SizedBox(width: 44, child: leading),
                    Expanded(
                      child: Text(
                        title!,
                        style: AppTypography.surfaceTitle.copyWith(
                          color: StyleConstants.deepGreen,
                        ),
                        textAlign: TextAlign.center,
                      ),
                    ),
                    const SizedBox(width: 44),
                  ],
                ),
              ),
              if (subtitle != null) ...[
                const SizedBox(height: 6),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 24),
                  child: Text(
                    subtitle!,
                    style: AppTypography.supporting.copyWith(
                      color: StyleConstants.mutedInkColor,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
              ],
            ] else
              const SizedBox(height: 8),
            Flexible(
              fit: FlexFit.loose,
              child: SingleChildScrollView(
                primary: false,
                padding: contentPadding,
                child: child,
              ),
            ),
            if (actions != null)
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
                child: actions!,
              ),
          ],
        ),
      ),
    );
  }
}

class AppDialogActions extends StatelessWidget {
  const AppDialogActions({super.key, required this.children, this.spacing = 10})
    : assert(spacing >= 0);

  final List<Widget> children;
  final double spacing;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final textScale = MediaQuery.textScalerOf(context).scale(14) / 14;
        final useColumn = constraints.maxWidth < 280 || textScale > 1.25;

        if (useColumn) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var i = 0; i < children.length; i++) ...[
                if (i > 0) SizedBox(height: spacing),
                children[i],
              ],
            ],
          );
        }

        return Row(
          children: [
            for (var i = 0; i < children.length; i++) ...[
              if (i > 0) SizedBox(width: spacing),
              Expanded(child: children[i]),
            ],
          ],
        );
      },
    );
  }
}

Future<T?> showAppDialog<T>(
  BuildContext context, {
  required WidgetBuilder builder,
  bool barrierDismissible = true,
  Color? barrierColor,
  double maxWidth = 420,
  EdgeInsets insetPadding = const EdgeInsets.symmetric(
    horizontal: 24,
    vertical: 24,
  ),
}) {
  assert(maxWidth > 0);
  return showDialog<T>(
    context: context,
    barrierDismissible: barrierDismissible,
    barrierColor:
        barrierColor ??
        StyleConstants.shadowColor.withValues(
          alpha: StyleConstants.isDarkMode ? 0.58 : 0.22,
        ),
    builder: (dialogContext) => PointerInterceptor(
      child: Dialog(
        elevation: 0,
        backgroundColor: Colors.transparent,
        insetPadding: insetPadding,
        child: ConstrainedBox(
          constraints: BoxConstraints(maxWidth: maxWidth),
          child: SizedBox(
            width: double.infinity,
            child: builder(dialogContext),
          ),
        ),
      ),
    ),
  );
}
