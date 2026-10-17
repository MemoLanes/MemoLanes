import 'dart:math' as math;
import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart' as legacy show Theme;
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

enum AppDialogSurfaceStyle { solid, glass }

/// Reading-sized forms/lists can use more space than a short prompt or picker.
enum AppDialogSize { compact, content }

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
        color: backgroundColor ?? context.appColors.canvasColor,
        borderRadius: borderRadius,
        border: Border.all(color: context.appColors.lineColor),
        boxShadow: [
          BoxShadow(
            color: context.appColors.shadowColor.withValues(alpha: shadowAlpha),
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

class AppDialogContent extends StatelessWidget {
  const AppDialogContent({
    super.key,
    required this.child,
    this.title,
    this.subtitle,
    this.leading,
    this.actions,
    this.showHeader = true,
    this.contentPadding = const EdgeInsets.fromLTRB(20, 12, 20, 18),
  });

  final Widget child;
  final String? title;
  final String? subtitle;
  final Widget? leading;
  final Widget? actions;
  final bool showHeader;
  final EdgeInsetsGeometry contentPadding;

  @override
  Widget build(BuildContext context) {
    return Column(
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
                      color: context.appColors.deepGreen,
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
                  color: context.appColors.mutedInkColor,
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
    );
  }
}

class AppDialogActions extends StatelessWidget {
  const AppDialogActions({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Row(
      spacing: 10,
      children: [for (final child in children) Expanded(child: child)],
    );
  }
}

/// Shared modal entry: callers provide content, and this owns the route,
/// surface, keyboard constraints and native safe region. Use AppMenu for anchors.
Future<T?> showAppDialog<T>(
  BuildContext context, {
  required WidgetBuilder builder,
  bool barrierDismissible = true,
  Color? barrierColor,
  AppDialogSize size = AppDialogSize.compact,
  EdgeInsets insetPadding = const EdgeInsets.symmetric(
    horizontal: 24,
    vertical: 24,
  ),
  Widget Function(Widget)? surfaceBuilder,
}) {
  final themes = InheritedTheme.capture(
    from: context,
    to: Navigator.of(context, rootNavigator: true).context,
  );
  return WoltModalSheet.show<T>(
    context: context,
    useRootNavigator: true,
    useSafeArea: false,
    enableDrag: false,
    showDragHandle: false,
    barrierDismissible: barrierDismissible,
    modalBarrierColor:
        barrierColor ??
        context.appColors.shadowColor.withValues(
          alpha: lerpDouble(0.22, 0.58, context.appColors.darkProgress)!,
        ),
    // Preserve showDialog's native foldable behavior. DuoDeviceProvider clears
    // engine features on actual Duo, where AppModalRegion owns placement.
    modalDecorator: (child) => themes.wrap(
      // Wolt still reads the SDK Material theme and localizations.
      // ignore: deprecated_member_use
      MaterialUiCompatibilityBridge(
        child: Builder(
          builder: (context) {
            final theme = legacy.Theme.of(context);
            return legacy.Theme(
              data: theme.copyWith(
                extensions: [
                  ...theme.extensions.values,
                  const WoltModalSheetThemeData(
                    modalElevation: 0,
                    clipBehavior: Clip.none,
                    mainContentScrollPhysics: NeverScrollableScrollPhysics(),
                  ),
                ],
              ),
              child: DisplayFeatureSubScreen(
                child: AppModalRegion(child: child),
              ),
            );
          },
        ),
      ),
    ),
    pageContentDecorator: (child) => PointerInterceptor(child: child),
    modalTypeBuilder: (context) => _AppDialogType(
      size,
      insetPadding,
      DialogTheme.of(context).shape ??
          const RoundedRectangleBorder(
            borderRadius: BorderRadius.all(Radius.circular(28)),
          ),
      surfaceBuilder,
    ),
    pageListBuilder: (_) => [
      SliverWoltModalSheetPage(
        hasTopBarLayer: false,
        hasSabGradient: false,
        backgroundColor: Colors.transparent,
        surfaceTintColor: Colors.transparent,
        mainContentSliversBuilder: (_) => [
          // Wolt owns the available viewport. The card keeps its fixed header
          // and actions, and scrolls only its body inside that constraint.
          SliverLayoutBuilder(
            builder: (context, constraints) => SliverToBoxAdapter(
              child: ConstrainedBox(
                constraints: BoxConstraints(
                  maxHeight: constraints.viewportMainAxisExtent,
                ),
                child: builder(context),
              ),
            ),
          ),
        ],
      ),
    ],
  );
}

/// Product dimensions; route, keyboard and transition behavior belong to Wolt.
class _AppDialogType extends WoltDialogType {
  const _AppDialogType(
    this.size,
    this.insets,
    ShapeBorder shape,
    this.surfaceBuilder,
  ) : super(shapeBorder: shape);

  final AppDialogSize size;
  final EdgeInsets insets;
  final Widget Function(Widget)? surfaceBuilder;

  @override
  BoxConstraints layoutModal(Size availableSize) {
    // Preserve the reading budget before reducing the outer margin in a
    // narrow pane. Wolt supplies this modal's actual available dimensions.
    final gap = math.max(
      8.0,
      (availableSize.width - StyleConstants.contentMinWidth) / 2,
    );
    final padding = insets.copyWith(
      left: math.min(insets.left, gap),
      right: math.min(insets.right, gap),
    );
    final budget = AppLayout.contentConstraints(
      availableSize,
      padding: padding,
      maxWidth: size == AppDialogSize.compact
          ? StyleConstants.dialogCompactMaxWidth
          : StyleConstants.pageContentMaxWidth,
    );
    return budget.tighten(width: budget.maxWidth);
  }

  @override
  Widget decoratePageContent(
    BuildContext context,
    Widget child,
    bool useSafeArea,
  ) => surfaceBuilder?.call(child) ?? AppDialogSurface(child: child);
}
