import 'package:memolanes/theme/app_colors.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

enum CapsuleBarSurfaceStyle { solid, mapGlass }

class CapsuleBarConstants {
  CapsuleBarConstants._();

  static const double barContentHeight = 48.0;
  static const double barBottomInset = 4.0;
  static const double maxSafeTop = 80.0;
  static const double pillRadius = 18.0;
  static const double iconButtonSize = 36.0;
}

/// Capsule-style bar content: back button, optional title pill, optional more button.
/// Handles horizontal safe area; vertical safe area belongs to the outer bar.
class CapsuleBarContent extends StatelessWidget {
  const CapsuleBarContent({
    super.key,
    required this.showOnlyBackButton,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreMenuContent,
    this.moreIcon,
    this.foregroundColor,
    this.pillColor,
    this.subtitleFg,
    this.surfaceStyle = CapsuleBarSurfaceStyle.solid,
    this.showTitleBackground = true,
  });

  final bool showOnlyBackButton;
  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;

  /// If set, the more button is wrapped with [CustomPopup] and shows this content on tap.
  final Widget? moreMenuContent;
  final Widget? moreIcon;
  final Color? foregroundColor;
  final Color? pillColor;
  final Color? subtitleFg;
  final CapsuleBarSurfaceStyle surfaceStyle;
  final bool showTitleBackground;

  Color _fg(BuildContext context) =>
      foregroundColor ?? context.appColors.inkColor;
  Color _pill(BuildContext context) =>
      pillColor ?? context.appColors.surfaceColor;
  Color _subFg(BuildContext context) =>
      subtitleFg ?? context.appColors.mutedInkColor;

  Widget _pillButton(
    BuildContext context,
    Widget icon,
    VoidCallback? onPressed,
  ) {
    return Material(
      color: _pill(context),
      shape: const CircleBorder(),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onPressed,
        customBorder: const CircleBorder(),
        child: SizedBox(
          width: CapsuleBarConstants.iconButtonSize,
          height: CapsuleBarConstants.iconButtonSize,
          child: Center(
            child: IconTheme.merge(
              data: IconThemeData(color: _fg(context), size: 20),
              child: icon,
            ),
          ),
        ),
      ),
    );
  }

  Widget _titleContent(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        if (title != null && title!.isNotEmpty)
          Text(
            title!,
            style: AppTypography.subpageTitle.copyWith(color: _fg(context)),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.center,
          ),
        if (subtitle != null && subtitle!.isNotEmpty) ...[
          const SizedBox(height: 1),
          Text(
            subtitle!,
            style: AppTypography.micro.copyWith(color: _subFg(context)),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.center,
          ),
        ],
      ],
    );
  }

  Widget _titlePill(BuildContext context) {
    final titleContent = _titleContent(context);
    if (!showTitleBackground) return titleContent;

    if (surfaceStyle == CapsuleBarSurfaceStyle.mapGlass) {
      final hasSubtitle = subtitle != null && subtitle!.isNotEmpty;
      return LiquidGlassSurface(
        borderRadius: BorderRadius.circular(CapsuleBarConstants.pillRadius),
        backgroundAlpha: 0.72,
        borderAlpha: 0.86,
        blurSigma: 18,
        reflectionAlpha: 0,
        shadowAlpha: 0.12,
        shadowBlurRadius: 18,
        shadowSpreadRadius: 0,
        shadowOffset: const Offset(0, 6),
        padding: EdgeInsets.symmetric(
          horizontal: 14,
          vertical: hasSubtitle ? 3 : 7,
        ),
        child: titleContent,
      );
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: BoxDecoration(
        color: _pill(context),
        borderRadius: BorderRadius.circular(CapsuleBarConstants.pillRadius),
      ),
      child: titleContent,
    );
  }

  @override
  Widget build(BuildContext context) {
    final onBackCallback = onBack ?? () => Navigator.maybePop(context);
    final safeAreaPadding = MediaQuery.paddingOf(context);
    return SizedBox(
      height: CapsuleBarConstants.barContentHeight,
      child: Padding(
        padding: EdgeInsets.only(
          left: safeAreaPadding.left + 8.0,
          right: safeAreaPadding.right + 8.0,
        ),
        child: Row(
          children: [
            if (surfaceStyle == CapsuleBarSurfaceStyle.mapGlass)
              MapGlassBackButton(onPressed: onBackCallback)
            else
              _pillButton(
                context,
                const Icon(Icons.arrow_back_ios_new, size: 20),
                onBackCallback,
              ),
            if (!showOnlyBackButton) ...[
              const SizedBox(width: 12),
              Expanded(child: Center(child: _titlePill(context))),
              const SizedBox(width: 12),
              if (moreMenuContent != null)
                CustomPopup(
                  position: PopupPosition.bottom,
                  contentRadius: StyleConstants.overlayFloatingRadius,
                  barrierColor: Colors.transparent,
                  backgroundColorBuilder: _pill,
                  contentPadding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  content: PointerInterceptor(child: moreMenuContent!),
                  child: _pillButton(
                    context,
                    moreIcon ?? const Icon(Icons.more_horiz, size: 24),
                    null,
                  ),
                )
              else if (onMoreTap != null)
                _pillButton(
                  context,
                  moreIcon ?? const Icon(Icons.more_horiz, size: 24),
                  onMoreTap,
                )
              else
                SizedBox(
                  width: surfaceStyle == CapsuleBarSurfaceStyle.mapGlass
                      ? 42
                      : CapsuleBarConstants.iconButtonSize,
                ),
            ],
          ],
        ),
      ),
    );
  }
}
