import 'package:memolanes/theme/app_colors.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_bar_content.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// Floating capsule bar for map pages; use [CapsuleStyleAppBar] in a Scaffold.
class CapsuleStyleOverlayAppBar extends StatelessWidget {
  const CapsuleStyleOverlayAppBar.overlayBar({
    super.key,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreMenuContent,
    this.moreIcon,
    this.showOnlyBackButton = false,
    this.surfaceStyle = CapsuleBarSurfaceStyle.solid,
  });

  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreMenuContent;
  final Widget? moreIcon;
  final bool showOnlyBackButton;
  final CapsuleBarSurfaceStyle surfaceStyle;

  @override
  Widget build(BuildContext context) {
    final layout = AppLayoutScope.of(context);
    final region = layout.headerBounds;
    final back = layout.detachedBackBounds;
    final topInset = layout.headerTopInset;
    final content = CapsuleBarContent(
      showOnlyBackButton: showOnlyBackButton,
      showBackButton: back == null,
      title: title,
      subtitle: subtitle,
      onBack: onBack,
      onMoreTap: onMoreTap,
      moreMenuContent: moreMenuContent,
      moreIcon: moreIcon,
      foregroundColor: context.appColors.inkColor,
      pillColor: context.appColors.surfaceColor,
      subtitleFg: context.appColors.mutedInkColor,
      surfaceStyle: surfaceStyle,
    );
    return Stack(
      children: [
        Positioned(
          top: region.top,
          left: region.left,
          right: layout.size.width - region.right,
          child: Container(
            height:
                topInset +
                CapsuleBarConstants.barContentHeight +
                CapsuleBarConstants.barBottomInset,
            color: Colors.transparent,
            child: MediaQuery(
              data: layout.headerMedia(MediaQuery.of(context)),
              child: Padding(
                padding: EdgeInsets.only(
                  top: topInset,
                  bottom: CapsuleBarConstants.barBottomInset,
                ),
                child: content,
              ),
            ),
          ),
        ),
        if (back != null)
          Positioned.fromRect(
            rect: back,
            child: PointerInterceptor(
              child: content.buildBackButton(
                context,
                expandedTouchTarget: true,
              ),
            ),
          ),
      ],
    );
  }
}
