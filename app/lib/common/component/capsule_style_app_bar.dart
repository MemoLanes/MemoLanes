import 'package:memolanes/theme/app_colors.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/capsule_style_bar_content.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

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
    final detached = AppLayout.fromContext(context).detachedBackBounds != null;
    final content = CapsuleBarContent(
      showOnlyBackButton: false,
      showBackButton: !detached,
      title: title,
      subtitle: subtitle,
      onBack: onBack,
      onMoreTap: onMoreTap,
      moreIcon: moreIcon,
      foregroundColor: foregroundColor ?? context.appColors.inkColor,
      pillColor: context.appColors.surfaceColor,
      subtitleFg: context.appColors.mutedInkColor,
      showTitleBackground: showTitleBackground,
    );

    return DetachedAppBarAction(
      detached: detached,
      actionBuilder: (context) =>
          content.buildBackButton(context, expandedTouchTarget: true),
      child: Container(
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
          padding: EdgeInsets.fromLTRB(
            padding.left + StyleConstants.overlayBarHorizontalInset,
            topInset,
            padding.right + StyleConstants.overlayBarHorizontalInset,
            CapsuleBarConstants.barBottomInset,
          ),
          child: MediaQuery.removePadding(
            context: context,
            removeLeft: true,
            removeTop: true,
            removeRight: true,
            removeBottom: true,
            child: content,
          ),
        ),
      ),
    );
  }
}

/// The native navigation column can extend below Scaffold's app-bar slot.
/// A portal keeps the action owned by its page and below subsequent routes.
class DetachedAppBarAction extends StatefulWidget {
  const DetachedAppBarAction({
    super.key,
    required this.detached,
    required this.actionBuilder,
    required this.child,
  });

  final bool detached;
  final WidgetBuilder actionBuilder;
  final Widget child;

  @override
  State<DetachedAppBarAction> createState() => _DetachedAppBarActionState();
}

class _DetachedAppBarActionState extends State<DetachedAppBarAction> {
  final _controller = OverlayPortalController();
  bool _syncScheduled = false;

  bool get _visible {
    final route = ModalRoute.of(context);
    return widget.detached &&
        route?.isCurrent == true &&
        route?.willHandlePopInternally != true;
  }

  @override
  Widget build(BuildContext context) {
    final visible = _visible;
    if (_controller.isShowing != visible && !_syncScheduled) {
      _syncScheduled = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _syncScheduled = false;
        if (!mounted) return;
        final show = _visible;
        if (show == _controller.isShowing) return;
        show ? _controller.show() : _controller.hide();
      });
    }
    return OverlayPortal.overlayChildLayoutBuilder(
      controller: _controller,
      overlayLocation: OverlayChildLocation.rootOverlay,
      overlayChildBuilder: (context, info) {
        final bounds = AppLayout.fromContext(
          context,
          size: info.overlaySize,
        ).detachedBackBounds;
        if (bounds == null || !_visible) {
          return const SizedBox.shrink();
        }
        return Positioned.fromRect(
          rect: bounds,
          child: PointerInterceptor(child: widget.actionBuilder(context)),
        );
      },
      child: widget.child,
    );
  }
}
