import 'package:flutter/material.dart';
import 'package:flutter_appbar/flutter_appbar.dart' as fappbar;
import 'package:memolanes/common/component/capsule_style_bar_content.dart';

/// Capsule-style app bar with two usage modes:
/// 1. **Normal**: use as [Scaffold.appBar], occupying space below the status bar.
/// 2. **Overlay**: use [CapsuleStyleOverlayAppBar.connection] to wrap page content; the bar is pinned by [flutter_appbar] and can stay fixed while content scrolls.
class CapsuleStyleOverlayAppBar extends StatelessWidget
    implements PreferredSizeWidget {
  const CapsuleStyleOverlayAppBar({
    super.key,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
    this.showOnlyBackButton = false,
  });

  final bool showOnlyBackButton;
  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;

  @override
  Size get preferredSize =>
      const Size.fromHeight(CapsuleBarConstants.barContentHeight +
          CapsuleBarConstants.barBottomInset +
          CapsuleBarConstants.maxSafeTop);

  /// Returns a transparent bar that can be placed in a [Stack] (non-layout, floats over content), e.g. with [SlidingUpPanel].
  /// If [moreMenuContent] is set, the more button shows it via [CustomPopup] on tap.
  static Widget overlayBar({
    Key? key,
    String? title,
    String? subtitle,
    VoidCallback? onBack,
    VoidCallback? onMoreTap,
    Widget? moreMenuContent,
    Widget? moreIcon,
    bool showOnlyBackButton = false,
  }) {
    return _OverlayBarOnly(
      key: key,
      title: title,
      subtitle: subtitle,
      onBack: onBack,
      onMoreTap: onMoreTap,
      moreMenuContent: moreMenuContent,
      moreIcon: moreIcon,
      showOnlyBackButton: showOnlyBackButton,
    );
  }

  /// Overlay mode: uses [flutter_appbar]'s [AppBarConnection] to pin the bar at the top with [child] below.
  /// [child] is wrapped in [CustomScrollView] + [SliverFillRemaining] so scroll behavior and layout are correct.
  static Widget connection({
    Key? key,
    required Widget child,
    String? title,
    String? subtitle,
    VoidCallback? onBack,
    VoidCallback? onMoreTap,
    Widget? moreIcon,
    Color? backgroundColor,
    Color? foregroundColor,
    bool showOnlyBackButton = false,
  }) {
    return _CapsuleOverlayConnection(
      key: key,
      child: child,
      title: title,
      subtitle: subtitle,
      onBack: onBack,
      onMoreTap: onMoreTap,
      moreIcon: moreIcon,
      backgroundColor: backgroundColor,
      foregroundColor: foregroundColor,
      showOnlyBackButton: showOnlyBackButton,
    );
  }

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    final barColor = backgroundColor ?? CapsuleBarConstants.defaultBackground;
    final isLight = barColor.computeLuminance() > 0.5;
    final pillColor = isLight
        ? CapsuleBarConstants.lightPillBackground
        : CapsuleBarConstants.defaultPill;
    final subtitleFg = isLight
        ? CapsuleBarConstants.subtitleColorLight
        : CapsuleBarConstants.defaultSubtitleFg;
    final borderColor = isLight
        ? CapsuleBarConstants.barBorderColorLight
        : CapsuleBarConstants.barBorderColor;

    return Container(
      height: topInset +
          CapsuleBarConstants.barContentHeight +
          CapsuleBarConstants.barBottomInset,
      decoration: BoxDecoration(
        color: barColor,
        border: Border(
          bottom: BorderSide(color: borderColor, width: 0.5),
        ),
      ),
      child: Padding(
        padding: EdgeInsets.only(
            top: topInset, bottom: CapsuleBarConstants.barBottomInset),
        child: CapsuleBarContent(
          showOnlyBackButton: showOnlyBackButton,
          title: title,
          subtitle: subtitle,
          onBack: onBack,
          onMoreTap: onMoreTap,
          moreIcon: moreIcon,
          foregroundColor:
              foregroundColor ?? CapsuleBarConstants.defaultForeground,
          pillColor: pillColor,
          subtitleFg: subtitleFg,
        ),
      ),
    );
  }
}

class _OverlayBarOnly extends StatelessWidget {
  const _OverlayBarOnly({
    super.key,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreMenuContent,
    this.moreIcon,
    this.showOnlyBackButton = false,
  });

  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreMenuContent;
  final Widget? moreIcon;
  final bool showOnlyBackButton;

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    return Positioned(
      top: 0,
      left: 0,
      right: 0,
      child: Container(
        height: topInset +
            CapsuleBarConstants.barContentHeight +
            CapsuleBarConstants.barBottomInset,
        color: Colors.transparent,
        child: Padding(
          padding: EdgeInsets.only(
              top: topInset, bottom: CapsuleBarConstants.barBottomInset),
          child: CapsuleBarContent(
            showOnlyBackButton: showOnlyBackButton,
            title: title,
            subtitle: subtitle,
            onBack: onBack,
            onMoreTap: onMoreTap,
            moreMenuContent: moreMenuContent,
            moreIcon: moreIcon,
            foregroundColor: CapsuleBarConstants.defaultForeground,
            pillColor: CapsuleBarConstants.defaultPill,
            subtitleFg: CapsuleBarConstants.defaultSubtitleFg,
          ),
        ),
      ),
    );
  }
}

class _CapsuleOverlayConnection extends StatelessWidget {
  const _CapsuleOverlayConnection({
    super.key,
    required this.child,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
    this.showOnlyBackButton = false,
  });

  final Widget child;
  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;
  final bool showOnlyBackButton;

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    final totalBarHeight = topInset +
        CapsuleBarConstants.barContentHeight +
        CapsuleBarConstants.barBottomInset;

    final barBody = Container(
      color: Colors.transparent,
      child: Padding(
        padding: EdgeInsets.only(
            top: topInset, bottom: CapsuleBarConstants.barBottomInset),
        child: CapsuleBarContent(
          showOnlyBackButton: showOnlyBackButton,
          title: title,
          subtitle: subtitle,
          onBack: onBack,
          onMoreTap: onMoreTap,
          moreIcon: moreIcon,
          foregroundColor:
              foregroundColor ?? CapsuleBarConstants.defaultForeground,
          pillColor: CapsuleBarConstants.defaultPill,
          subtitleFg: CapsuleBarConstants.defaultSubtitleFg,
        ),
      ),
    );

    return fappbar.AppBarConnection(
      appBars: [
        fappbar.AppBar(
          behavior: const fappbar.AbsoluteAppBarBehavior(),
          minExtent: totalBarHeight,
          maxExtent: totalBarHeight,
          body: barBody,
        ),
      ],
      child: CustomScrollView(
        slivers: [
          SliverFillRemaining(
            hasScrollBody: false,
            child: child,
          ),
        ],
      ),
    );
  }
}
