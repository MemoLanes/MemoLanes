import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_bar_content.dart';

/// Capsule-style app bar: circular back button, center title pill (title + optional subtitle), circular more button.
/// Use as [Scaffold.appBar]; occupies space below the status bar. Suited for dark backgrounds (e.g. scaffoldBackgroundColor 0xFF141414).
class CapsuleStyleAppBar extends StatelessWidget implements PreferredSizeWidget {
  const CapsuleStyleAppBar({
    super.key,
    required this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
  });

  final String title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;

  @override
  Size get preferredSize => const Size.fromHeight(
      CapsuleBarConstants.barContentHeight +
          CapsuleBarConstants.barBottomInset +
          CapsuleBarConstants.maxSafeTop);

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    final bg = backgroundColor ?? CapsuleBarConstants.defaultBackground;
    final isLight = bg.computeLuminance() > 0.5;
    final pillColor =
        isLight ? CapsuleBarConstants.lightPillBackground : CapsuleBarConstants.defaultPill;
    final subtitleFg =
        isLight ? CapsuleBarConstants.subtitleColorLight : CapsuleBarConstants.defaultSubtitleFg;
    final borderColor = isLight
        ? CapsuleBarConstants.barBorderColorLight
        : CapsuleBarConstants.barBorderColor;

    return Container(
      height: topInset +
          CapsuleBarConstants.barContentHeight +
          CapsuleBarConstants.barBottomInset,
      decoration: BoxDecoration(
        color: bg,
        border: Border(
          bottom: BorderSide(color: borderColor, width: 0.5),
        ),
      ),
      child: Padding(
        padding: EdgeInsets.only(top: topInset, bottom: CapsuleBarConstants.barBottomInset),
        child: CapsuleBarContent(
          showOnlyBackButton: false,
          title: title,
          subtitle: subtitle,
          onBack: onBack,
          onMoreTap: onMoreTap,
          moreIcon: moreIcon,
          foregroundColor: foregroundColor ?? CapsuleBarConstants.defaultForeground,
          pillColor: pillColor,
          subtitleFg: subtitleFg,
        ),
      ),
    );
  }
}
