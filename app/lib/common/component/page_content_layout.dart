import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/constants/style_constants.dart';

/// Consumes resolved page insets once, without imposing a content width.
class PageContentInsets extends StatelessWidget {
  const PageContentInsets({
    super.key,
    required this.child,
    this.enabled = true,
    this.maintainViewPadding = false,
    this.ownsChildMargins = false,
  }) : _menuLayout = null;

  /// Main menu content also clears the app's navigation and vertical insets.
  const PageContentInsets.menu({
    super.key,
    required AppLayout layout,
    required this.child,
    this.enabled = true,
  }) : _menuLayout = layout,
       maintainViewPadding = true,
       ownsChildMargins = false;

  final Widget child;
  final bool enabled;

  /// Frames preserve physical horizontal insets even if padding was removed
  /// by an ancestor; scroll bodies use the remaining MediaQuery.padding.
  final bool maintainViewPadding;
  final bool ownsChildMargins;
  final AppLayout? _menuLayout;

  @override
  Widget build(BuildContext context) {
    final owner = PageInsetsScope.maybeOf(context);
    final layout = _menuLayout ?? AppLayoutScope.of(context);
    final media = MediaQuery.of(context);
    final resolved = _menuLayout == null
        ? layout.contentInsets(media, maintainViewPadding: maintainViewPadding)
        : layout.menuInsets;
    final horizontal = enabled && owner?.horizontal != true && resolved != null;
    final vertical = enabled && _menuLayout != null && owner?.vertical != true;
    final padding = EdgeInsets.fromLTRB(
      horizontal ? resolved.left : 0,
      vertical ? resolved!.top : 0,
      horizontal ? resolved.right : 0,
      vertical ? resolved!.bottom : 0,
    );
    return Padding(
      padding: padding,
      child: MediaQuery(
        data: media.removePadding(
          removeLeft: horizontal,
          removeRight: horizontal,
          removeTop: vertical,
          removeBottom: vertical,
        ),
        child: PageInsetsScope(
          horizontal: horizontal || owner?.horizontal == true,
          vertical: vertical || owner?.vertical == true,
          ownsChildMargins: ownsChildMargins || owner?.ownsChildMargins == true,
          child: child,
        ),
      ),
    );
  }
}

/// The shared content column used by settings and dashboards.
class PageContentFrame extends StatelessWidget {
  const PageContentFrame({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.symmetric(horizontal: 16),
    this.framed = true,
    this.consumeChildMargins = false,
    this.maxWidth = maxContentWidth,
  });

  static const double maxContentWidth = StyleConstants.pageContentMaxWidth;

  final Widget child;
  final EdgeInsetsGeometry padding;
  final bool framed;
  final bool consumeChildMargins;
  final double maxWidth;

  @override
  Widget build(BuildContext context) {
    return PageContentInsets(
      enabled: framed,
      maintainViewPadding: true,
      ownsChildMargins: framed && consumeChildMargins,
      child: Padding(
        padding: framed ? padding : EdgeInsets.zero,
        child: Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: framed ? maxWidth : double.infinity,
            child: child,
          ),
        ),
      ),
    );
  }
}

class PageContentLayout extends StatelessWidget {
  const PageContentLayout({
    super.key,
    required this.children,
    this.topPadding = 12,
    this.bottomPadding = 24,
    this.framePadding = const EdgeInsets.symmetric(horizontal: 16),
    this.framed = true,
    this.consumeChildMargins = false,
  });

  const PageContentLayout.dashboard({
    super.key,
    required this.children,
    this.topPadding = 0,
    this.bottomPadding = 24,
  }) : framePadding = const EdgeInsets.symmetric(horizontal: 8),
       framed = null,
       consumeChildMargins = true;

  final List<Widget> children;
  final double topPadding;
  final double bottomPadding;
  final EdgeInsetsGeometry framePadding;
  final bool? framed;
  final bool consumeChildMargins;
  @override
  Widget build(BuildContext context) {
    final layout = AppLayoutScope.of(context);
    final contentWidth = framed == null
        ? layout.dashboardContentWidth
        : PageContentFrame.maxContentWidth;
    final useFrame = framed ?? contentWidth.isFinite;
    return MlSingleChildScrollView(
      padding: EdgeInsets.only(
        top: topPadding + (framed == null ? layout.dashboardTopPadding : 0),
        bottom: bottomPadding,
      ),
      children: [
        PageContentFrame(
          padding: framePadding,
          maxWidth: contentWidth,
          framed: useFrame,
          consumeChildMargins: consumeChildMargins,
          child: Column(
            crossAxisAlignment: useFrame
                ? CrossAxisAlignment.start
                : CrossAxisAlignment.center,
            children: children,
          ),
        ),
      ],
    );
  }
}
