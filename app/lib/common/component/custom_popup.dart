import 'dart:math';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

enum _ArrowDirection { top, bottom, left, right }

enum PopupPosition { auto, top, bottom, left, right }

class CustomPopup extends StatefulWidget {
  final GlobalKey? anchorKey;
  final Widget content;
  final Widget child;
  final bool isLongPress;
  final Color? backgroundColor;
  final Color? arrowColor;
  final Color? barrierColor;
  final bool showArrow;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;
  final VoidCallback? onBeforePopup;
  final VoidCallback? onAfterPopup;
  final bool rootNavigator;
  final PopupPosition position;

  /// 水平/垂直偏移量
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
    this.arrowColor,
    this.showArrow = false,
    this.barrierColor,
    this.contentPadding = const EdgeInsets.all(8),
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
        arrowColor: widget.arrowColor,
        showArrow: widget.showArrow,
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
  final GlobalKey arrowKey;
  final _ArrowDirection arrowDirection;
  final double arrowHorizontal;
  final double arrowVertical;
  final Color? backgroundColor;
  final Color? arrowColor;
  final bool showArrow;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;

  const _PopupContent({
    Key? key,
    required this.child,
    required this.childKey,
    required this.arrowKey,
    required this.arrowHorizontal,
    this.arrowVertical = 0,
    required this.showArrow,
    this.arrowDirection = _ArrowDirection.top,
    this.backgroundColor,
    this.arrowColor,
    this.contentRadius,
    required this.contentPadding,
    this.contentDecoration,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        Container(
          key: childKey,
          padding: contentPadding,
          margin: const EdgeInsets.symmetric(vertical: 10, horizontal: 10).copyWith(
            top: arrowDirection == _ArrowDirection.bottom ? 0 : null,
            bottom: arrowDirection == _ArrowDirection.top ? 0 : null,
            left: arrowDirection == _ArrowDirection.right ? 0 : null,
            right: arrowDirection == _ArrowDirection.left ? 0 : null,
          ),
          constraints: const BoxConstraints(minWidth: 50),
          decoration: contentDecoration ??
              BoxDecoration(
                color: backgroundColor ?? Colors.white,
                borderRadius: BorderRadius.circular(contentRadius ?? 10),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withOpacity(0.1),
                    blurRadius: 10,
                  ),
                ],
              ),
          child: child,
        ),
        if (showArrow)
          Positioned(
            // top: arrowDirection == _ArrowDirection.top ? 2 : null,
            bottom: arrowDirection == _ArrowDirection.bottom ? 2 : null,
            left: arrowDirection == _ArrowDirection.top ||
                arrowDirection == _ArrowDirection.bottom
                ? arrowHorizontal
                : null,
            top: arrowDirection == _ArrowDirection.left ||
                arrowDirection == _ArrowDirection.right
                ? arrowVertical
                : null,
            right: arrowDirection == _ArrowDirection.left ? 2 : null,
            child: RotatedBox(
              key: arrowKey,
              quarterTurns: _getArrowQuarterTurns(arrowDirection),
              child: CustomPaint(
                size: const Size(16, 8),
                painter: _TrianglePainter(color: arrowColor ?? Colors.white),
              ),
            ),
          ),
      ],
    );
  }

  int _getArrowQuarterTurns(_ArrowDirection direction) {
    switch (direction) {
      case _ArrowDirection.top:
        return 2;
      case _ArrowDirection.bottom:
        return 4;
      case _ArrowDirection.left:
      case _ArrowDirection.right:
        return 1; // 左右箭头旋转90°
    }
  }
}

class _TrianglePainter extends CustomPainter {
  final Color color;

  const _TrianglePainter({required this.color});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint();
    final path = Path();
    paint.isAntiAlias = true;
    paint.color = color;

    path.moveTo(0, 0);
    path.lineTo(size.width / 2, size.height);
    path.lineTo(size.width, 0);
    path.close();

    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) => true;
}

class _PopupRoute extends PopupRoute<void> {
  final Rect targetRect;
  final PopupPosition position;
  final Widget child;
  final double? horizontalOffset;
  final double? verticalOffset;

  final GlobalKey _childKey = GlobalKey();
  final GlobalKey _arrowKey = GlobalKey();
  final Color? backgroundColor;
  final Color? arrowColor;
  final bool showArrow;
  final Color? barriersColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;

  static const double _margin = 10;

  double _maxHeight = 0;
  _ArrowDirection _arrowDirection = _ArrowDirection.top;
  double _arrowHorizontal = 0;
  double _arrowVertical = 0;
  double _scaleAlignDx = 0.5;
  double _scaleAlignDy = 0.5;
  double? _top;
  double? _bottom;
  double? _left;
  double? _right;

  final Duration animationDuration;
  final Curve animationCurve;

  _PopupRoute({
    required this.child,
    required this.targetRect,
    this.position = PopupPosition.auto,
    this.horizontalOffset,
    this.verticalOffset,
    this.backgroundColor,
    this.arrowColor,
    required this.showArrow,
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
        _arrowDirection = _ArrowDirection.bottom;
        _scaleAlignDy = 1;
        _left = targetRect.center.dx - childRect.width / 2 + (horizontalOffset ?? 0);
        _right = null;
        _arrowHorizontal = childRect.width / 2 - 8;
        break;
      case PopupPosition.bottom:
        _top = targetRect.bottom + (verticalOffset ?? 0);
        _bottom = null;
        _arrowDirection = _ArrowDirection.top;
        _scaleAlignDy = 0;
        _left = targetRect.center.dx - childRect.width / 2 + (horizontalOffset ?? 0);
        _right = null;
        _arrowHorizontal = childRect.width / 2 - 8;
        break;
      case PopupPosition.left:
        _left = targetRect.left - childRect.width + (horizontalOffset ?? 0);
        _right = null;

        // 垂直居中
        _top = targetRect.center.dy - childRect.height / 2 + (verticalOffset ?? 0);
        _bottom = null;

        _arrowDirection = _ArrowDirection.right;
        _arrowVertical = childRect.height / 2 - 4; // 箭头垂直居中
        break;

      case PopupPosition.right:
        _left = targetRect.right + (horizontalOffset ?? 0);
        _right = null;

        // 垂直居中
        _top = targetRect.center.dy - childRect.height / 2 + (verticalOffset ?? 0);
        _bottom = null;

        _arrowDirection = _ArrowDirection.left;
        _arrowVertical = childRect.height / 2 - 4; // 箭头垂直居中
        break;

      case PopupPosition.auto:
        if (screenSize.height - targetRect.bottom > targetRect.top) {
          // 下方空间大
          _top = targetRect.bottom + (verticalOffset ?? 0);
          _bottom = null;
          _arrowDirection = _ArrowDirection.top;
          _arrowHorizontal = childRect.width / 2 - 8;
        } else {
          _top = null;
          _bottom = screenSize.height - targetRect.top + (verticalOffset ?? 0);
          _arrowDirection = _ArrowDirection.bottom;
          _arrowHorizontal = childRect.width / 2 - 8;
        }
        _left = targetRect.center.dx - childRect.width / 2 + (horizontalOffset ?? 0);
        _right = null;
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
      arrowKey: _arrowKey,
      arrowHorizontal: _arrowHorizontal,
      arrowVertical: _arrowVertical,
      arrowDirection: _arrowDirection,
      backgroundColor: backgroundColor,
      arrowColor: arrowColor,
      showArrow: showArrow,
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
            constraints: BoxConstraints(maxWidth: 300, maxHeight: 300),
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
