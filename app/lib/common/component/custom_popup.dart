import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:memolanes/theme/app_colors.dart';

enum PopupPosition { auto, top, bottom, left, right }

class CustomPopup extends StatefulWidget {
  final GlobalKey? anchorKey;
  final Widget? content;

  /// Builds content in the popup route so inherited theme changes update it.
  final WidgetBuilder? contentBuilder;
  final Widget child;
  final bool isLongPress;
  final Color? backgroundColor;
  final Color? barrierColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;
  // Resolve theme-dependent surfaces inside the route, including while open.
  final BoxDecoration Function(BuildContext)? contentDecorationBuilder;
  final Color Function(BuildContext)? backgroundColorBuilder;
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
    this.content,
    this.contentBuilder,
    required this.child,
    this.anchorKey,
    this.isLongPress = false,
    this.backgroundColor,
    this.barrierColor,
    this.contentPadding = const EdgeInsets.all(16),
    this.contentRadius,
    this.contentDecoration,
    this.contentDecorationBuilder,
    this.backgroundColorBuilder,
    this.onBeforePopup,
    this.onAfterPopup,
    this.rootNavigator = false,
    this.position = PopupPosition.auto,
    this.horizontalOffset,
    this.verticalOffset,
    this.animationDuration = const Duration(milliseconds: 150),
    this.animationCurve = Curves.easeInOut,
  }) : assert((content == null) != (contentBuilder == null));

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
            barriersColor:
                widget.barrierColor ??
                context.appColors.shadowColor.withValues(
                  alpha: ui.lerpDouble(
                    0.1,
                    0.28,
                    context.appColors.darkProgress,
                  )!,
                ),
            contentPadding: widget.contentPadding,
            contentRadius: widget.contentRadius,
            contentDecoration: widget.contentDecoration,
            contentDecorationBuilder: widget.contentDecorationBuilder,
            backgroundColorBuilder: widget.backgroundColorBuilder,
            position: widget.position,
            horizontalOffset: widget.horizontalOffset,
            verticalOffset: widget.verticalOffset,
            animationDuration: widget.animationDuration,
            animationCurve: widget.animationCurve,
            child: widget.contentBuilder == null
                ? widget.content!
                : Builder(builder: widget.contentBuilder!),
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
  final Color? backgroundColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;
  // Resolve theme-dependent surfaces inside the route, including while open.
  final BoxDecoration Function(BuildContext)? contentDecorationBuilder;
  final Color Function(BuildContext)? backgroundColorBuilder;

  const _PopupContent({
    required this.child,
    this.backgroundColor,
    required this.contentPadding,
    this.contentRadius,
    this.contentDecoration,
    this.contentDecorationBuilder,
    this.backgroundColorBuilder,
  });

  @override
  Widget build(BuildContext context) {
    final resolvedBackground =
        backgroundColorBuilder?.call(context) ?? backgroundColor;
    return Container(
      padding: contentPadding,
      constraints: const BoxConstraints(minWidth: 50),
      decoration:
          contentDecorationBuilder?.call(context) ??
          contentDecoration ??
          BoxDecoration(
            color: resolvedBackground ?? context.appColors.canvasColor,
            borderRadius: BorderRadius.circular(contentRadius ?? 16),
            border: resolvedBackground == null
                ? Border.all(color: context.appColors.lineColor)
                : null,
            boxShadow: [
              BoxShadow(
                color: context.appColors.shadowColor.withValues(
                  alpha: resolvedBackground == null
                      ? context.appColors.popupShadowAlpha
                      : ui.lerpDouble(
                          0.1,
                          0.34,
                          context.appColors.darkProgress,
                        )!,
                ),
                blurRadius: resolvedBackground == null ? 18 : 10,
                offset: resolvedBackground == null
                    ? const Offset(0, 6)
                    : Offset.zero,
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

  final Color? backgroundColor;
  final Color? barriersColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;
  // Resolve theme-dependent surfaces inside the route, including while open.
  final BoxDecoration Function(BuildContext)? contentDecorationBuilder;
  final Color Function(BuildContext)? backgroundColorBuilder;

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
    this.contentDecorationBuilder,
    this.backgroundColorBuilder,
    required this.animationDuration,
    this.animationCurve = Curves.easeInOut,
  });

  @override
  Color? get barrierColor => barriersColor;
  @override
  bool get barrierDismissible => true;
  @override
  String? get barrierLabel => 'Popup';

  @override
  Widget buildPage(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
  ) {
    return child;
  }

  @override
  Widget buildTransitions(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) {
    child = _PopupContent(
      backgroundColor: backgroundColor,
      contentPadding: contentPadding,
      contentRadius: contentRadius,
      contentDecoration: contentDecoration,
      contentDecorationBuilder: contentDecorationBuilder,
      backgroundColorBuilder: backgroundColorBuilder,
      child: child,
    );

    final curvedAnimation = CurvedAnimation(
      parent: animation,
      curve: animationCurve,
    );

    final scaleAlignment = switch (position) {
      PopupPosition.top => Alignment.bottomCenter,
      PopupPosition.bottom => Alignment.topCenter,
      PopupPosition.left => Alignment.centerRight,
      PopupPosition.right => Alignment.centerLeft,
      PopupPosition.auto =>
        targetRect.top > MediaQuery.sizeOf(context).height - targetRect.bottom
            ? Alignment.bottomCenter
            : Alignment.topCenter,
    };

    return CustomSingleChildLayout(
      delegate: _PopupLayoutDelegate(
        targetRect: targetRect,
        position: position,
        padding: MediaQuery.paddingOf(context),
        horizontalOffset: horizontalOffset ?? 0,
        verticalOffset: verticalOffset ?? 0,
      ),
      child: FadeTransition(
        opacity: curvedAnimation,
        child: ScaleTransition(
          alignment: scaleAlignment,
          scale: curvedAnimation,
          child: Material(color: Colors.transparent, child: child),
        ),
      ),
    );
  }

  @override
  Duration get transitionDuration => animationDuration;
}

class _PopupLayoutDelegate extends SingleChildLayoutDelegate {
  const _PopupLayoutDelegate({
    required this.targetRect,
    required this.position,
    required this.padding,
    required this.horizontalOffset,
    required this.verticalOffset,
  });

  static const _edgeMargin = 8.0;

  final Rect targetRect;
  final PopupPosition position;
  final EdgeInsets padding;
  final double horizontalOffset;
  final double verticalOffset;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      BoxConstraints(
        maxWidth: math.max(
          0,
          constraints.maxWidth - padding.horizontal - 2 * _edgeMargin,
        ),
        maxHeight: math.max(
          0,
          constraints.maxHeight - padding.vertical - 2 * _edgeMargin,
        ),
      );

  @override
  Offset getPositionForChild(Size size, Size childSize) {
    // Use the current child size so live content keeps its anchor and center.
    final above =
        position == PopupPosition.top ||
        (position == PopupPosition.auto &&
            targetRect.top > size.height - targetRect.bottom);
    final x = switch (position) {
      PopupPosition.left =>
        targetRect.left - childSize.width + horizontalOffset,
      PopupPosition.right => targetRect.right + horizontalOffset,
      _ => targetRect.center.dx - childSize.width / 2 + horizontalOffset,
    };
    final y = switch (position) {
      PopupPosition.left || PopupPosition.right =>
        targetRect.center.dy - childSize.height / 2 + verticalOffset,
      _ when above => targetRect.top - childSize.height - verticalOffset,
      _ => targetRect.bottom + verticalOffset,
    };
    return Offset(
      x.clamp(
        padding.left + _edgeMargin,
        size.width - childSize.width - padding.right - _edgeMargin,
      ),
      y.clamp(
        padding.top + _edgeMargin,
        size.height - childSize.height - padding.bottom - _edgeMargin,
      ),
    );
  }

  @override
  bool shouldRelayout(_PopupLayoutDelegate oldDelegate) =>
      targetRect != oldDelegate.targetRect ||
      position != oldDelegate.position ||
      padding != oldDelegate.padding ||
      horizontalOffset != oldDelegate.horizontalOffset ||
      verticalOffset != oldDelegate.verticalOffset;
}
