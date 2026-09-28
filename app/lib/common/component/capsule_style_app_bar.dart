import 'package:memolanes/theme/app_colors.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_bar_content.dart';

/// Capsule-style app bar: circular back button, center title pill (title + optional subtitle), circular more button.
/// Use as [Scaffold.appBar]; occupies space below the status bar.
class CapsuleStyleAppBar extends StatelessWidget
    implements PreferredSizeWidget {
  const CapsuleStyleAppBar({
    super.key,
    required this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
    this.showTitleBackground = true,
  });

  final String title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;
  final bool showTitleBackground;

  @override
  Size get preferredSize => const Size.fromHeight(
    CapsuleBarConstants.barContentHeight +
        CapsuleBarConstants.barBottomInset +
        CapsuleBarConstants.maxSafeTop,
  );

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;

    return Container(
      height:
          topInset +
          CapsuleBarConstants.barContentHeight +
          CapsuleBarConstants.barBottomInset,
      decoration: BoxDecoration(
        color: backgroundColor ?? context.appColors.canvasColor,
        border: Border(
          bottom: BorderSide(color: context.appColors.lineColor, width: 0.5),
        ),
      ),
      child: Padding(
        padding: EdgeInsets.only(
          top: topInset,
          bottom: CapsuleBarConstants.barBottomInset,
        ),
        child: CapsuleBarContent(
          showOnlyBackButton: false,
          title: title,
          subtitle: subtitle,
          onBack: onBack,
          onMoreTap: onMoreTap,
          moreIcon: moreIcon,
          foregroundColor: foregroundColor ?? context.appColors.inkColor,
          pillColor: context.appColors.surfaceColor,
          subtitleFg: context.appColors.mutedInkColor,
          showTitleBackground: showTitleBackground,
        ),
      ),
    );
  }
}
