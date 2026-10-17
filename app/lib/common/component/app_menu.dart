import 'dart:math' as math;
import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

enum AppMenuPosition {
  /// Align leading edges and use the larger space above or below the anchor.
  auto,
  top,
  bottom,

  /// Open beside the anchor, toward the wider side of its usable region.
  side,
}

/// Existing app surfaces with MenuAnchor's animation, focus and dismissal.
/// Only product attachment and clear-region constraints are adapted here.
class AppMenu extends StatefulWidget {
  const AppMenu({
    super.key,
    required this.menuBuilder,
    required this.child,
    this.controller,
    this.position = AppMenuPosition.auto,
    this.offset = Offset.zero,
    this.margin = const EdgeInsets.all(8),
    this.surfaceBuilder,
    this.style,
  });

  final WidgetBuilder menuBuilder;
  final Widget child;
  final MenuController? controller;
  final AppMenuPosition position;
  final Offset offset;
  final EdgeInsets margin;
  final Widget Function(BuildContext, Widget)? surfaceBuilder;
  final MenuStyle? style;

  @override
  State<AppMenu> createState() => _AppMenuState();
}

class _AppMenuState extends State<AppMenu> {
  final _ownedController = MenuController();
  final _menuKey = GlobalKey();
  LocalHistoryEntry? _history;
  Rect _anchor = Rect.zero;
  Size? _overlaySize;
  Size _menuSize = Size.zero;
  bool _placementScheduled = false;
  AnimationStatus _animationStatus = AnimationStatus.dismissed;

  MenuController get _controller => widget.controller ?? _ownedController;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _schedulePlacement();
  }

  @override
  void didUpdateWidget(covariant AppMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    _schedulePlacement();
  }

  void _schedulePlacement() {
    if (_placementScheduled || !_controller.isOpen) return;
    _placementScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _placementScheduled = false;
      _updatePlacement();
    });
  }

  void _updatePlacement() {
    if (!mounted || !_controller.isOpen) return;
    final anchor = context.findRenderObject() as RenderBox?;
    final overlay =
        Overlay.of(context).context.findRenderObject() as RenderBox?;
    final menu = _menuKey.currentContext?.findRenderObject() as RenderBox?;
    if (anchor == null || overlay == null || !anchor.hasSize) return;
    final rect =
        anchor.localToGlobal(Offset.zero, ancestor: overlay) & anchor.size;
    final menuSize = menu != null && menu.hasSize ? menu.size : _menuSize;
    if (rect == _anchor &&
        overlay.size == _overlaySize &&
        menuSize == _menuSize) {
      return;
    }
    setState(() {
      _anchor = rect;
      _overlaySize = overlay.size;
      _menuSize = menuSize;
    });
  }

  void _opened() {
    _updatePlacement();
    _schedulePlacement();
    if (_history != null) return;
    late final LocalHistoryEntry history;
    history = LocalHistoryEntry(
      impliesAppBarDismissal: false,
      onRemove: () {
        if (_history != history) return;
        _history = null;
        _controller.close();
      },
    );
    _history = history;
    ModalRoute.of(context)?.addLocalHistoryEntry(history);
  }

  void _closed() {
    final history = _history;
    _history = null;
    history?.remove();
  }

  @override
  void dispose() {
    _closed();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final media = MediaQuery.of(context);
    // Scaffold consumes the keyboard inset for its body; the menu lives in
    // the full Navigator overlay and must still exclude that keyboard area.
    final menuMedia = media.copyWith(
      viewInsets: MediaQuery.viewInsetsOf(Overlay.of(context).context),
    );
    final size = _overlaySize ?? media.size;
    final layout = AppLayout.fromContext(
      context,
      media: menuMedia,
      size: Size(
        size.width,
        math.max(0, size.height - menuMedia.viewInsets.bottom),
      ),
    );
    final bounds = widget.margin.deflateRect(
      layout.menuRegionFor(
        _anchor.center,
        overlaySize: size,
        navigationBounds: AppLayoutScope.maybeOf(context)?.navigationBounds,
      ),
    );
    final above =
        widget.position == AppMenuPosition.top ||
        (widget.position == AppMenuPosition.auto &&
            _anchor.top - bounds.top >= bounds.bottom - _anchor.bottom);
    final left = _anchor.center.dx >= bounds.center.dx;
    final maxHeight = math.max(
      0.0,
      widget.position == AppMenuPosition.auto
          ? (above
                    ? _anchor.top - bounds.top
                    : bounds.bottom - _anchor.bottom) -
                widget.offset.dy
          : bounds.height,
    );
    final rtl = Directionality.of(context) == TextDirection.rtl;
    final alignment = switch (widget.position) {
      AppMenuPosition.side =>
        left ? Alignment.centerLeft : Alignment.centerRight,
      AppMenuPosition.auto => Alignment(rtl ? 1 : -1, above ? -1 : 1),
      _ => above ? Alignment.topCenter : Alignment.bottomCenter,
    };
    // Translate centered/side attachment into MenuAnchor's alignment offset.
    final offset = switch (widget.position) {
      AppMenuPosition.side => Offset(
        (left ? -_menuSize.width - widget.offset.dx : widget.offset.dx) +
            (rtl ? _menuSize.width : 0),
        -_menuSize.height / 2 + widget.offset.dy,
      ),
      _ => Offset(
        widget.offset.dx +
            (widget.position == AppMenuPosition.auto
                ? 0
                : (rtl ? _menuSize.width / 2 : -_menuSize.width / 2)),
        above ? -_menuSize.height - widget.offset.dy : widget.offset.dy,
      ),
    };
    // MenuAnchor limits size using reservedPadding, but positions against the
    // root overlay's safe area. Keep its target inside our clear region too.
    final origin =
        alignment.withinRect(_anchor) +
        offset -
        Offset(rtl ? _menuSize.width : 0, 0);
    final clampedOrigin = Offset(
      origin.dx.clamp(
        bounds.left,
        math.max(bounds.left, bounds.right - _menuSize.width),
      ),
      origin.dy.clamp(
        bounds.top,
        math.max(bounds.top, bounds.bottom - _menuSize.height),
      ),
    );
    final padding = EdgeInsets.fromLTRB(
      bounds.left,
      bounds.top,
      math.max(0, size.width - bounds.right),
      math.max(0, size.height - bounds.bottom),
    );
    return MenuAnchor(
      animated: true,
      controller: _controller,
      clipBehavior: Clip.none,
      crossAxisUnconstrained: false,
      consumeOutsideTap: true,
      reservedPadding: padding,
      alignmentOffset: offset + clampedOrigin - origin,
      style: MenuStyle(
        alignment: alignment,
        padding: const WidgetStatePropertyAll(EdgeInsets.zero),
        visualDensity: VisualDensity.standard,
        backgroundColor: const WidgetStatePropertyAll(Colors.transparent),
        surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
        elevation: const WidgetStatePropertyAll(0),
      ).merge(widget.style),
      onOpen: _opened,
      onClose: _closed,
      onAnimationStatusChanged: (status) => _animationStatus = status,
      builder: (_, controller, child) => GestureDetector(
        behavior: HitTestBehavior.translucent,
        onTap: () => _animationStatus.isForwardOrCompleted
            ? controller.close()
            : controller.open(),
        child: child,
      ),
      menuChildren: [
        NotificationListener<SizeChangedLayoutNotification>(
          onNotification: (_) {
            _schedulePlacement();
            return false;
          },
          child: SizeChangedLayoutNotifier(
            key: _menuKey,
            child: Builder(
              builder: (context) {
                final content = SingleChildScrollView(
                  primary: false,
                  child: widget.menuBuilder(context),
                );
                // The SDK scrolls menu items. Our glass surface stays fixed
                // while a long custom item scrolls inside its rounded border.
                return ConstrainedBox(
                  constraints: BoxConstraints(maxHeight: maxHeight),
                  child: Focus(
                    autofocus: true,
                    child: Semantics(
                      scopesRoute: true,
                      explicitChildNodes: true,
                      namesRoute: true,
                      label: MaterialLocalizations.of(context).popupMenuLabel,
                      child: PointerInterceptor(
                        child:
                            widget.surfaceBuilder?.call(context, content) ??
                            AppMenuSurface(child: content),
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
        ),
      ],
      child: widget.child,
    );
  }
}

/// Shared menu appearance, independent of anchor positioning.
class AppMenuSurface extends StatelessWidget {
  const AppMenuSurface({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(16),
    this.radius = 16,
    this.backgroundColor,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final double radius;
  final Color? backgroundColor;

  @override
  Widget build(BuildContext context) => Container(
    padding: padding,
    constraints: const BoxConstraints(minWidth: 50),
    decoration: BoxDecoration(
      color: backgroundColor ?? context.appColors.canvasColor,
      borderRadius: BorderRadius.circular(radius),
      border: backgroundColor == null
          ? Border.all(color: context.appColors.lineColor)
          : null,
      boxShadow: [
        BoxShadow(
          color: context.appColors.shadowColor.withValues(
            alpha: backgroundColor == null
                ? context.appColors.popupShadowAlpha
                : lerpDouble(0.1, 0.34, context.appColors.darkProgress)!,
          ),
          blurRadius: backgroundColor == null ? 18 : 10,
          offset: backgroundColor == null ? const Offset(0, 6) : Offset.zero,
        ),
      ],
    ),
    child: child,
  );
}
