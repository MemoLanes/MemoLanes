import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:memolanes/constants/style_constants.dart';

enum PopupPosition { auto, top, bottom, left, right }

/// Visual presets for [CustomPopup].
///
/// The default black popup keeps the previous appearance. The white preset
/// shares the translucent surface and border used by floating glass controls.
enum CustomPopupTheme { black, white }

extension CustomPopupThemeStyle on CustomPopupTheme {
  Color get backgroundColor => switch (this) {
        CustomPopupTheme.black => Colors.black,
        // Matches FrostedBarContainer's default white background at 80%.
        CustomPopupTheme.white => const Color(0xCCFFFFFF),
      };

  Color get barrierColor => switch (this) {
        CustomPopupTheme.black => Colors.black.withValues(alpha: 0.1),
        CustomPopupTheme.white => Colors.transparent,
      };

  Border? get border => switch (this) {
        CustomPopupTheme.black => null,
        CustomPopupTheme.white => const Border.fromBorderSide(
            BorderSide(color: StyleConstants.glassControlBorderColor),
          ),
      };

  Color get contentColor => switch (this) {
        CustomPopupTheme.black => StyleConstants.glassControlContentColor,
        CustomPopupTheme.white => const Color(0xDE000000),
      };

  Color get mutedContentColor => switch (this) {
        CustomPopupTheme.black => StyleConstants.glassControlMutedContentColor,
        CustomPopupTheme.white => const Color(0x99000000),
      };

  Color get accentColor => switch (this) {
        CustomPopupTheme.black => StyleConstants.defaultColor,
        CustomPopupTheme.white => StyleConstants.glassControlAccentColor,
      };

  Color get dividerColor => switch (this) {
        CustomPopupTheme.black => StyleConstants.glassControlBorderColor,
        CustomPopupTheme.white => const Color(0x1F000000),
      };

  List<BoxShadow> get boxShadow => switch (this) {
        CustomPopupTheme.black => [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.1),
              blurRadius: 10,
            ),
          ],
        CustomPopupTheme.white => const [],
      };
}

typedef CustomPopupBuilder = Widget Function(
  BuildContext context,
  VoidCallback show,
);

class CustomPopup extends StatefulWidget {
  final GlobalKey? anchorKey;
  final Widget content;
  final CustomPopupBuilder builder;
  final CustomPopupTheme theme;
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
    required this.builder,
    this.anchorKey,
    this.theme = CustomPopupTheme.black,
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
            theme: widget.theme,
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
    return widget.builder(context, show);
  }
}

class _PopupContent extends StatelessWidget {
  final Widget child;
  final GlobalKey childKey;
  final CustomPopupTheme theme;
  final Color? backgroundColor;
  final EdgeInsets contentPadding;
  final double? contentRadius;
  final BoxDecoration? contentDecoration;
  final ValueChanged<Size>? onSizeChanged;

  const _PopupContent({
    required this.child,
    required this.childKey,
    required this.theme,
    this.backgroundColor,
    required this.contentPadding,
    this.contentRadius,
    this.contentDecoration,
    this.onSizeChanged,
  });

  @override
  Widget build(BuildContext context) {
    return _SizeChangeObserver(
      onSizeChanged: onSizeChanged,
      child: Container(
        key: childKey,
        padding: contentPadding,
        constraints: const BoxConstraints(minWidth: 50),
        decoration: contentDecoration ??
            BoxDecoration(
              color: backgroundColor ?? theme.backgroundColor,
              borderRadius: BorderRadius.circular(contentRadius ?? 10),
              border: theme.border,
              boxShadow: theme.boxShadow,
            ),
        child: child,
      ),
    );
  }
}

class _SizeChangeObserver extends SingleChildRenderObjectWidget {
  const _SizeChangeObserver({
    required super.child,
    this.onSizeChanged,
  });

  final ValueChanged<Size>? onSizeChanged;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _SizeChangeRenderObject(onSizeChanged);

  @override
  void updateRenderObject(
    BuildContext context,
    covariant _SizeChangeRenderObject renderObject,
  ) {
    renderObject.onSizeChanged = onSizeChanged;
  }
}

class _SizeChangeRenderObject extends RenderProxyBox {
  _SizeChangeRenderObject(this.onSizeChanged);

  ValueChanged<Size>? onSizeChanged;
  Size? _lastSize;

  @override
  void performLayout() {
    super.performLayout();
    if (_lastSize == size) return;

    _lastSize = size;
    SchedulerBinding.instance.addPostFrameCallback((_) {
      if (attached) onSizeChanged?.call(size);
    });
  }
}

class _PopupRoute extends PopupRoute<void> {
  final Rect targetRect;
  final PopupPosition position;
  final Widget child;
  final CustomPopupTheme theme;
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
    required this.theme,
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
  Color? get barrierColor => barriersColor ?? theme.barrierColor;
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

  static const double _kEdgeMargin = 8.0;

  void _calculateChildOffset(Rect? childRect) {
    if (childRect == null) return;
    _top = null;
    _bottom = null;
    _left = null;
    _right = null;
    _scaleAlignDx = 0.5;
    _scaleAlignDy = 0.5;

    final view = ui.PlatformDispatcher.instance.views.first;
    final media = MediaQueryData.fromView(view);
    final screenSize = media.size;
    final padding = media.padding;

    switch (position) {
      case PopupPosition.top:
        _top = null;
        _bottom = screenSize.height - targetRect.top + (verticalOffset ?? 0);
        _scaleAlignDy = 1;
        _left = targetRect.center.dx -
            childRect.width / 2 +
            (horizontalOffset ?? 0);
        break;
      case PopupPosition.bottom:
        _top = targetRect.bottom + (verticalOffset ?? 0);
        _scaleAlignDy = 0;
        _left = targetRect.center.dx -
            childRect.width / 2 +
            (horizontalOffset ?? 0);
        break;
      case PopupPosition.left:
        _right = screenSize.width - targetRect.left - (horizontalOffset ?? 0);
        _top =
            targetRect.center.dy - childRect.height / 2 + (verticalOffset ?? 0);
        _scaleAlignDx = 1;
        break;
      case PopupPosition.right:
        _left = targetRect.right + (horizontalOffset ?? 0);
        _top =
            targetRect.center.dy - childRect.height / 2 + (verticalOffset ?? 0);
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
        _left = targetRect.center.dx -
            childRect.width / 2 +
            (horizontalOffset ?? 0);
        break;
    }

    if (_left != null) {
      _left = _left!.clamp(
        padding.left + _kEdgeMargin,
        screenSize.width - childRect.width - padding.right - _kEdgeMargin,
      );
    }
    if (_right != null) {
      _right = _right!.clamp(
        padding.right + _kEdgeMargin,
        screenSize.width - childRect.width - padding.left - _kEdgeMargin,
      );
    }
    if (_top != null) {
      _top = _top!.clamp(
        padding.top + _kEdgeMargin,
        screenSize.height - childRect.height - padding.bottom - _kEdgeMargin,
      );
    }
    if (_bottom != null) {
      final maxBottom =
          screenSize.height - childRect.height - padding.top - _kEdgeMargin;
      _bottom = _bottom!.clamp(padding.bottom + _kEdgeMargin, maxBottom);
    }
  }

  BoxConstraints _screenPopupConstraints(BuildContext context) {
    final media = MediaQuery.of(context);
    final maxAvailableWidth = math.max(
      0.0,
      media.size.width -
          media.padding.left -
          media.padding.right -
          _kEdgeMargin * 2,
    );
    final maxAvailableHeight = math.max(
      0.0,
      media.size.height -
          media.padding.top -
          media.padding.bottom -
          _kEdgeMargin * 2,
    );

    return BoxConstraints(
      maxWidth: maxAvailableWidth,
      maxHeight: maxAvailableHeight,
    );
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
      theme: theme,
      backgroundColor: backgroundColor,
      contentPadding: contentPadding,
      contentRadius: contentRadius,
      contentDecoration: contentDecoration,
      onSizeChanged: (size) {
        _calculateChildOffset(Offset.zero & size);
        changedInternalState();
      },
      child: child,
    );

    final curvedAnimation =
        CurvedAnimation(parent: animation, curve: animationCurve);

    return Stack(
      children: [
        Positioned(
          left: _left,
          right: _right,
          top: _top,
          bottom: _bottom,
          child: ConstrainedBox(
            constraints: _screenPopupConstraints(context),
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
