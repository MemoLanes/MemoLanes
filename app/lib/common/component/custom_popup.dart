import 'dart:ui';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

enum PopupPosition { auto, top, bottom, left, right }

class CustomPopup extends StatefulWidget {
  final GlobalKey? anchorKey;
  final Widget content;
  final Widget child;
  final bool isLongPress;
  final Color? backgroundColor;
  final Color? barrierColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;
  final VoidCallback? onBeforePopup;
  final VoidCallback? onAfterPopup;
  final bool rootNavigator;
  final PopupPosition position;
  final double? horizontalOffset;
  final double? verticalOffset;

  final Duration animationDuration;
  final Curve animationCurve;

  const CustomPopup({
    super.key,
    required this.content,
    required this.child,
    this.anchorKey,
    this.isLongPress = false,
    this.backgroundColor,
    this.barrierColor,
    this.contentPadding = const EdgeInsets.all(16),
    this.contentRadius,
    this.contentDecoration,
    this.onBeforePopup,
    this.onAfterPopup,
    this.rootNavigator = false,
    this.position = PopupPosition.auto,
    this.horizontalOffset,
    this.verticalOffset,
    this.animationDuration = const Duration(milliseconds: 150),
    this.animationCurve = Curves.easeInOut,
  });

  @override
  State<CustomPopup> createState() => CustomPopupState();
}

class CustomPopupState extends State<CustomPopup> {
  void show() {
    final anchor = widget.anchorKey?.currentContext ?? context;
    final renderBox = anchor.findRenderObject() as RenderBox?;
    if (renderBox == null) return;
    final offset = renderBox.localToGlobal(renderBox.paintBounds.topLeft);

    widget.onBeforePopup?.call();

    Navigator.of(context, rootNavigator: widget.rootNavigator)
        .push(
      _PopupRoute(
        targetRect: offset & renderBox.paintBounds.size,
        backgroundColor: widget.backgroundColor,
        barriersColor: widget.barrierColor,
        contentPadding: widget.contentPadding,
        contentRadius: widget.contentRadius,
        contentDecoration: widget.contentDecoration,
        position: widget.position,
        horizontalOffset: widget.horizontalOffset,
        verticalOffset: widget.verticalOffset,
        animationDuration: widget.animationDuration,
        animationCurve: widget.animationCurve,
        child: widget.content,
      ),
    )
        .then((value) => widget.onAfterPopup?.call());
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onLongPress: widget.isLongPress ? () => show() : null,
      onTapUp: !widget.isLongPress ? (_) => show() : null,
      child: widget.child,
    );
  }
}

class _PopupContent extends StatelessWidget {
  final Widget child;
  final GlobalKey childKey;
  final Color? backgroundColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;

  const _PopupContent({
    Key? key,
    required this.child,
    required this.childKey,
    this.backgroundColor,
    required this.contentPadding,
    this.contentRadius,
    this.contentDecoration,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      key: childKey,
      padding: contentPadding,
      constraints: const BoxConstraints(minWidth: 50),
      decoration: contentDecoration ??
          BoxDecoration(
            color: backgroundColor ?? Colors.black,
            borderRadius: BorderRadius.circular(contentRadius ?? 10),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withOpacity(0.1),
                blurRadius: 10,
              ),
            ],
          ),
      child: child,
    );
  }
}

class _PopupRoute extends PopupRoute<void> {
  final Rect targetRect;
  final PopupPosition position;
  final Widget child;
  final double? horizontalOffset;
  final double? verticalOffset;

  final GlobalKey _childKey = GlobalKey();
  final Color? backgroundColor;
  final Color? barriersColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;

  double? _top;
  double? _bottom;
  double? _left;
  double? _right;
  double _scaleAlignDx = 0.5;
  double _scaleAlignDy = 0.5;

  final Duration animationDuration;
  final Curve animationCurve;

  _PopupRoute({
    required this.child,
    required this.targetRect,
    this.position = PopupPosition.auto,
    this.horizontalOffset,
    this.verticalOffset,
    this.backgroundColor,
    this.barriersColor,
    required this.contentPadding,
    this.contentRadius,
    this.contentDecoration,
    required this.animationDuration,
    this.animationCurve = Curves.easeInOut,
  });

  @override
  Color? get barrierColor => barriersColor ?? Colors.black.withOpacity(0.1);
  @override
  bool get barrierDismissible => true;
  @override
  String? get barrierLabel => 'Popup';

  @override
  TickerFuture didPush() {
    super.offstage = true;
    SchedulerBinding.instance.addPostFrameCallback((_) {
      final childRect = _getRect(_childKey);
      _calculateChildOffset(childRect);
      super.offstage = false;
    });
    return super.didPush();
  }

  Rect? _getRect(GlobalKey key) {
    final currentContext = key.currentContext;
    final renderBox = currentContext?.findRenderObject() as RenderBox?;
    if (renderBox == null || currentContext == null) return null;
    final offset = renderBox.localToGlobal(renderBox.paintBounds.topLeft);
    return offset & renderBox.paintBounds.size;
  }

  void _calculateChildOffset(Rect? childRect) {
    if (childRect == null) return;

    final screenSize = MediaQueryData.fromWindow(WidgetsBinding.instance.window).size;

    switch (position) {
      case PopupPosition.top:
        _top = null;
        _bottom = screenSize.height - targetRect.top + (verticalOffset ?? 0);
        _scaleAlignDy = 1;
        _left = targetRect.center.dx - childRect.width / 2 + (horizontalOffset ?? 0);
        break;
      case PopupPosition.bottom:
        _top = targetRect.bottom + (verticalOffset ?? 0);
        _scaleAlignDy = 0;
        _left = targetRect.center.dx - childRect.width / 2 + (horizontalOffset ?? 0);
        break;
      case PopupPosition.left:
        _left = targetRect.left - childRect.width + (horizontalOffset ?? 0);
        _top = targetRect.center.dy - childRect.height / 2 + (verticalOffset ?? 0);
        _scaleAlignDx = 1;
        break;
      case PopupPosition.right:
        _left = targetRect.right + (horizontalOffset ?? 0);
        _top = targetRect.center.dy - childRect.height / 2 + (verticalOffset ?? 0);
        _scaleAlignDx = 0;
        break;
      case PopupPosition.auto:
        if (screenSize.height - targetRect.bottom > targetRect.top) {
          _top = targetRect.bottom + (verticalOffset ?? 0);
          _scaleAlignDy = 0;
        } else {
          _bottom = screenSize.height - targetRect.top + (verticalOffset ?? 0);
          _scaleAlignDy = 1;
        }
        _left = targetRect.center.dx - childRect.width / 2 + (horizontalOffset ?? 0);
        break;
    }
  }

  @override
  Widget buildPage(BuildContext context, Animation<double> animation,
      Animation<double> secondaryAnimation) {
    return child;
  }

  @override
  Widget buildTransitions(BuildContext context, Animation<double> animation,
      Animation<double> secondaryAnimation, Widget child) {
    child = _PopupContent(
      childKey: _childKey,
      backgroundColor: backgroundColor,
      contentPadding: contentPadding,
      contentRadius: contentRadius,
      contentDecoration: contentDecoration,
      child: child,
    );

    final curvedAnimation = CurvedAnimation(parent: animation, curve: animationCurve);

    return Stack(
      children: [
        Positioned(
          left: _left,
          right: _right,
          top: _top,
          bottom: _bottom,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 300, maxHeight: 300),
            child: FadeTransition(
              opacity: curvedAnimation,
              child: ScaleTransition(
                alignment: FractionalOffset(_scaleAlignDx, _scaleAlignDy),
                scale: curvedAnimation,
                child: Material(
                  color: Colors.transparent,
                  child: child,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  @override
  Duration get transitionDuration => animationDuration;
}
