// 内联自 advance_ruler_slider 包，含 animateInitialScroll 支持。

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// A controller for the RulerScale widget, allowing programmatic control.
class RulerScaleController {
  _RulerScaleState? _state;

  void _attach(_RulerScaleState state) {
    _state = state;
  }

  void _detach() {
    _state = null;
  }

  void jumpToValue(double targetValue) {
    _state?.jumpToValue(targetValue);
  }
}

/// 刻度尺组件：可水平/垂直滑动，支持自定义刻度与标签，中心指示当前值。
class RulerScale extends StatefulWidget {
  final double minValue;
  final double maxValue;
  final double majorTickInterval;
  final Axis direction;
  final Color majorTickColor;
  final Color minorTickColor;
  final double majorTickWidth;
  final double minorTickWidth;
  final double majorTickLength;
  final double minorTickLength;
  final TextStyle labelStyle;
  final Color indicatorColor;
  final double indicatorWidth;
  final double rulerExtent;
  final ValueChanged<double>? onValueChanged;
  final VoidCallback? onScrollStart;
  final VoidCallback? onScrollEnd;
  final double initialValue;
  final double unitSpacing;
  final double step;
  final RulerScaleController? controller;
  final bool useScrollAnimation;
  /// 首次布局滚到 [initialValue] 时是否动画；false 则直接 jumpTo。
  final bool animateInitialScroll;
  final Duration scrollAnimationDuration;
  final Curve scrollAnimationCurve;
  final bool hapticFeedbackEnabled;
  final bool isReadOnly;
  final String Function(double value)? labelFormatter;
  final Color? activeRangeColor;
  final double activeRangeOpacity;
  final bool showBoundaryLabels;
  final ScrollPhysics? scrollPhysics;
  final Widget? customIndicator;
  final double labelOffset;
  final StrokeCap tickStrokeCap;
  final double? minScrollValue;
  final double? maxScrollValue;
  final BoxDecoration? decoration;
  final Color? selectedTickColor;
  final double? selectedTickWidth;
  final double? selectedTickLength;
  final bool showDefaultIndicator;

  const RulerScale({
    super.key,
    this.minValue = 0.0,
    this.maxValue = 100.0,
    this.majorTickInterval = 5,
    this.direction = Axis.horizontal,
    this.majorTickColor = Colors.black,
    this.minorTickColor = Colors.grey,
    this.majorTickWidth = 2.0,
    this.minorTickWidth = 1.0,
    this.majorTickLength = 20.0,
    this.minorTickLength = 10.0,
    this.labelStyle = const TextStyle(color: Colors.black, fontSize: 12.0),
    this.indicatorColor = Colors.red,
    this.indicatorWidth = 2.0,
    this.rulerExtent = 120.0,
    this.onValueChanged,
    this.onScrollStart,
    this.onScrollEnd,
    this.initialValue = 0.0,
    this.unitSpacing = 10.0,
    this.step = 1,
    this.controller,
    this.useScrollAnimation = true,
    this.animateInitialScroll = true,
    this.scrollAnimationDuration = const Duration(milliseconds: 300),
    this.scrollAnimationCurve = Curves.easeOutCubic,
    this.hapticFeedbackEnabled = true,
    this.isReadOnly = false,
    this.labelFormatter,
    this.activeRangeColor,
    this.activeRangeOpacity = 0.2,
    this.showBoundaryLabels = false,
    this.scrollPhysics,
    this.customIndicator,
    this.labelOffset = 5.0,
    this.tickStrokeCap = StrokeCap.round,
    this.minScrollValue,
    this.maxScrollValue,
    this.decoration,
    this.selectedTickColor = Colors.blue,
    this.selectedTickWidth = 4,
    this.selectedTickLength = 20,
    this.showDefaultIndicator = false,
  })  : assert(minValue < maxValue),
        assert(majorTickInterval > 0),
        assert(initialValue >= minValue && initialValue <= maxValue),
        assert(unitSpacing > 0),
        assert(step > 0),
        assert(activeRangeOpacity >= 0.0 && activeRangeOpacity <= 1.0),
        assert(minScrollValue == null || minScrollValue >= minValue),
        assert(maxScrollValue == null || maxScrollValue <= maxValue),
        assert(
          minScrollValue == null ||
              maxScrollValue == null ||
              minScrollValue < maxScrollValue,
        ),
        super();

  @override
  State<RulerScale> createState() => _RulerScaleState();
}

class _RulerScaleState extends State<RulerScale> {
  late ScrollController _scrollController;
  double _currentValue = 0.0;
  double _viewportDimension = 0.0;
  bool _isInitialScrollPerformed = false;

  @override
  void initState() {
    super.initState();
    _currentValue = widget.initialValue;
    _scrollController = ScrollController();
    widget.controller?._attach(this);
  }

  @override
  void dispose() {
    widget.controller?._detach();
    _scrollController.dispose();
    super.dispose();
  }

  double _calculateScrollOffsetForValue(double value) {
    final double pixelPositionRelativeToMin =
        (value - widget.minValue) * widget.unitSpacing;
    final double scrollOffset = pixelPositionRelativeToMin;
    final double effectiveMinScrollValue =
        widget.minScrollValue ?? widget.minValue;
    final double minAllowedPixelOffset =
        (effectiveMinScrollValue - widget.minValue) * widget.unitSpacing;
    final double totalRange = widget.maxValue - widget.minValue;
    final double totalContentLength =
        (totalRange * widget.unitSpacing) + _viewportDimension;
    final double actualMaxScrollOffset =
        totalContentLength - _viewportDimension;
    return scrollOffset.clamp(minAllowedPixelOffset, actualMaxScrollOffset);
  }

  void _scrollToInitialValue() {
    if (_viewportDimension == 0 || !_scrollController.hasClients) return;

    final double snappedInitialValue =
        (widget.initialValue / widget.step).round() * widget.step;
    final double clampedInitialValue = snappedInitialValue.clamp(
      widget.minScrollValue ?? widget.minValue,
      widget.maxScrollValue ?? widget.maxValue,
    );
    final double scrollOffset =
        _calculateScrollOffsetForValue(clampedInitialValue);

    if (widget.animateInitialScroll && widget.useScrollAnimation) {
      _scrollController.animateTo(
        scrollOffset,
        duration: widget.scrollAnimationDuration,
        curve: widget.scrollAnimationCurve,
      );
    } else {
      _scrollController.jumpTo(scrollOffset);
    }
    _updateValueAndNotify(clampedInitialValue);
  }

  void jumpToValue(double targetValue) {
    if (!_scrollController.hasClients || _viewportDimension <= 0) return;

    final double snappedTargetValue =
        (targetValue / widget.step).round() * widget.step;
    final double clampedTargetValue = snappedTargetValue.clamp(
      widget.minScrollValue ?? widget.minValue,
      widget.maxScrollValue ?? widget.maxValue,
    );
    final double scrollOffset =
        _calculateScrollOffsetForValue(clampedTargetValue);

    if (widget.useScrollAnimation) {
      _scrollController.animateTo(
        scrollOffset,
        duration: widget.scrollAnimationDuration,
        curve: widget.scrollAnimationCurve,
      );
    } else {
      _scrollController.jumpTo(scrollOffset);
    }
    _updateValueAndNotify(clampedTargetValue);
  }

  void _updateValueAndNotify(double newValue) {
    final double clampedNewValue = newValue.clamp(
      widget.minScrollValue ?? widget.minValue,
      widget.maxScrollValue ?? widget.maxValue,
    );
    if (clampedNewValue != _currentValue) {
      setState(() => _currentValue = clampedNewValue);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        widget.onValueChanged?.call(_currentValue);
      });
      if (widget.hapticFeedbackEnabled) {
        HapticFeedback.selectionClick();
      }
    }
  }

  bool _handleScrollNotification(ScrollNotification notification) {
    if (_viewportDimension == 0 || widget.unitSpacing == 0) return false;
    if (notification is ScrollStartNotification) {
      widget.onScrollStart?.call();
    }

    final double centerPixelInContent =
        notification.metrics.pixels + (_viewportDimension / 2);
    double calculatedValue = widget.minValue +
        ((centerPixelInContent - (_viewportDimension / 2)) /
            widget.unitSpacing);
    calculatedValue = (calculatedValue / widget.step).round() * widget.step;
    calculatedValue = calculatedValue.clamp(widget.minValue, widget.maxValue);

    if (calculatedValue != _currentValue) {
      setState(() => _currentValue = calculatedValue);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        widget.onValueChanged?.call(_currentValue);
      });
      if (widget.hapticFeedbackEnabled) {
        HapticFeedback.selectionClick();
      }
    }
    if (notification is ScrollEndNotification) {
      widget.onScrollEnd?.call();
    }
    return true;
  }

  @override
  Widget build(BuildContext context) {
    final double totalRange = widget.maxValue - widget.minValue;
    final double totalContentLength =
        (totalRange * widget.unitSpacing) + _viewportDimension;
    final double calculatedHeight = widget.rulerExtent;
    final double calculatedWidth = widget.rulerExtent;

    return Container(
      width: widget.direction == Axis.vertical ? calculatedWidth : null,
      height: widget.direction == Axis.horizontal ? calculatedHeight : null,
      decoration: widget.decoration,
      clipBehavior: widget.decoration?.borderRadius != null
          ? Clip.hardEdge
          : Clip.none,
      child: Stack(
        alignment: Alignment.center,
        children: [
          if (widget.activeRangeColor != null)
            CustomPaint(
              painter: _ActiveRangePainter(
                color: widget.activeRangeColor!
                    .withValues(alpha: widget.activeRangeOpacity),
                direction: widget.direction,
                highlightThickness: widget.indicatorWidth,
              ),
              size: widget.direction == Axis.horizontal
                  ? Size(double.infinity, widget.rulerExtent)
                  : Size(widget.rulerExtent, double.infinity),
            ),
          LayoutBuilder(
            builder: (BuildContext context, BoxConstraints constraints) {
              final double newViewportDimension =
                  widget.direction == Axis.horizontal
                      ? constraints.maxWidth
                      : constraints.maxHeight;

              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (newViewportDimension != _viewportDimension) {
                  setState(() => _viewportDimension = newViewportDimension);
                }
                if (!_isInitialScrollPerformed && _viewportDimension > 0) {
                  _scrollToInitialValue();
                  _isInitialScrollPerformed = true;
                }
              });

              return widget.direction == Axis.horizontal
                  ? SizedBox(
                      height: widget.rulerExtent,
                      child: NotificationListener<ScrollNotification>(
                        onNotification: _handleScrollNotification,
                        child: SingleChildScrollView(
                          controller: _scrollController,
                          scrollDirection: widget.direction,
                          physics: widget.isReadOnly
                              ? const NeverScrollableScrollPhysics()
                              : (widget.scrollPhysics ??
                                    const AlwaysScrollableScrollPhysics()),
                          child: CustomPaint(
                            size: Size(totalContentLength, widget.rulerExtent),
                            painter: _RulerPainter(
                              minValue: widget.minValue,
                              maxValue: widget.maxValue,
                              majorTickInterval: widget.majorTickInterval,
                              direction: widget.direction,
                              majorTickColor: widget.majorTickColor,
                              minorTickColor: widget.minorTickColor,
                              majorTickWidth: widget.majorTickWidth,
                              minorTickWidth: widget.minorTickWidth,
                              majorTickLength: widget.majorTickLength,
                              minorTickLength: widget.minorTickLength,
                              labelStyle: widget.labelStyle,
                              unitSpacing: _viewportDimension > 0
                                  ? widget.unitSpacing
                                  : 0.001,
                              viewportDimension: _viewportDimension,
                              labelFormatter: widget.labelFormatter,
                              showBoundaryLabels: widget.showBoundaryLabels,
                              labelOffset: widget.labelOffset,
                              tickStrokeCap: widget.tickStrokeCap,
                              currentValue: _currentValue,
                              selectedTickColor: widget.selectedTickColor,
                              selectedTickWidth: widget.selectedTickWidth,
                              selectedTickLength: widget.selectedTickLength,
                              step: widget.step,
                            ),
                          ),
                        ),
                      ),
                    )
                  : SizedBox(
                      width: widget.rulerExtent,
                      child: NotificationListener<ScrollNotification>(
                        onNotification: _handleScrollNotification,
                        child: SingleChildScrollView(
                          controller: _scrollController,
                          scrollDirection: widget.direction,
                          physics: widget.isReadOnly
                              ? const NeverScrollableScrollPhysics()
                              : (widget.scrollPhysics ??
                                    const AlwaysScrollableScrollPhysics()),
                          child: CustomPaint(
                            size: Size(widget.rulerExtent, totalContentLength),
                            painter: _RulerPainter(
                              minValue: widget.minValue,
                              maxValue: widget.maxValue,
                              majorTickInterval: widget.majorTickInterval,
                              direction: widget.direction,
                              majorTickColor: widget.majorTickColor,
                              minorTickColor: widget.minorTickColor,
                              majorTickWidth: widget.majorTickWidth,
                              minorTickWidth: widget.minorTickWidth,
                              majorTickLength: widget.majorTickLength,
                              minorTickLength: widget.minorTickLength,
                              labelStyle: widget.labelStyle,
                              unitSpacing: _viewportDimension > 0
                                  ? widget.unitSpacing
                                  : 0.001,
                              viewportDimension: _viewportDimension,
                              labelFormatter: widget.labelFormatter,
                              showBoundaryLabels: widget.showBoundaryLabels,
                              labelOffset: widget.labelOffset,
                              tickStrokeCap: widget.tickStrokeCap,
                              currentValue: _currentValue,
                              selectedTickColor: widget.selectedTickColor,
                              selectedTickWidth: widget.selectedTickWidth,
                              selectedTickLength: widget.selectedTickLength,
                              step: widget.step,
                            ),
                          ),
                        ),
                      ),
                    );
            },
          ),
          if (widget.showDefaultIndicator || widget.customIndicator != null)
            IgnorePointer(
              child: Align(
                alignment: Alignment.center,
                child: widget.customIndicator ??
                    (widget.showDefaultIndicator
                        ? (widget.direction == Axis.horizontal
                            ? Container(
                                width: widget.indicatorWidth,
                                height: widget.rulerExtent,
                                color: widget.indicatorColor,
                              )
                            : Container(
                                width: widget.rulerExtent,
                                height: widget.indicatorWidth,
                                color: widget.indicatorColor,
                              ))
                        : const SizedBox.shrink()),
              ),
            ),
        ],
      ),
    );
  }
}

class _RulerPainter extends CustomPainter {
  final double minValue;
  final double maxValue;
  final double majorTickInterval;
  final Axis direction;
  final Color majorTickColor;
  final Color minorTickColor;
  final double majorTickWidth;
  final double minorTickWidth;
  final double majorTickLength;
  final double minorTickLength;
  final TextStyle labelStyle;
  final double unitSpacing;
  final double viewportDimension;
  final String Function(double value)? labelFormatter;
  final bool showBoundaryLabels;
  final double labelOffset;
  final StrokeCap tickStrokeCap;
  final double currentValue;
  final Color? selectedTickColor;
  final double? selectedTickWidth;
  final double? selectedTickLength;
  final double step;

  _RulerPainter({
    required this.minValue,
    required this.maxValue,
    required this.majorTickInterval,
    required this.direction,
    required this.majorTickColor,
    required this.minorTickColor,
    required this.majorTickWidth,
    required this.minorTickWidth,
    required this.majorTickLength,
    required this.minorTickLength,
    required this.labelStyle,
    required this.unitSpacing,
    required this.viewportDimension,
    this.labelFormatter,
    this.showBoundaryLabels = false,
    this.labelOffset = 5.0,
    this.tickStrokeCap = StrokeCap.round,
    required this.currentValue,
    this.selectedTickColor,
    this.selectedTickWidth,
    this.selectedTickLength,
    required this.step,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final Paint majorTickPaint = Paint()
      ..color = majorTickColor
      ..strokeWidth = majorTickWidth
      ..strokeCap = tickStrokeCap;
    final Paint minorTickPaint = Paint()
      ..color = minorTickColor
      ..strokeWidth = minorTickWidth
      ..strokeCap = tickStrokeCap;

    final double totalRange = maxValue - minValue;
    if (totalRange <= 0 || unitSpacing == 0 || viewportDimension == 0) return;
    final double leadingPadding = viewportDimension / 2;

    String formatLabel(double value) {
      if (labelFormatter != null) return labelFormatter!(value);
      return value
          .toStringAsFixed(_getDecimalPlacesForLabels(majorTickInterval));
    }

    for (double value = minValue; value <= maxValue; value += step) {
      final double pixelPosition =
          (value - minValue) * unitSpacing + leadingPadding;
      const double epsilon = 1e-9;
      bool isMajorTick = (value % majorTickInterval).abs() < epsilon ||
          (majorTickInterval - (value % majorTickInterval)).abs() < epsilon;
      bool isSelectedTick = (value - currentValue).abs() < (step / 2);

      final Paint currentTickPaint;
      final double currentTickLength;
      if (isSelectedTick && selectedTickColor != null) {
        currentTickPaint = Paint()
          ..color = selectedTickColor!
          ..strokeWidth = selectedTickWidth ??
              (isMajorTick ? majorTickWidth : minorTickWidth)
          ..strokeCap = tickStrokeCap;
        currentTickLength = selectedTickLength ??
            (isMajorTick ? majorTickLength : minorTickLength);
      } else {
        currentTickPaint = isMajorTick ? majorTickPaint : minorTickPaint;
        currentTickLength = isMajorTick ? majorTickLength : minorTickLength;
      }

      if (direction == Axis.horizontal) {
        canvas.drawLine(
          Offset(pixelPosition, 0),
          Offset(pixelPosition, currentTickLength),
          currentTickPaint,
        );
        if (isMajorTick ||
            (showBoundaryLabels && (value == minValue || value == maxValue))) {
          final TextPainter textPainter = TextPainter(
            text: TextSpan(text: formatLabel(value), style: labelStyle),
            textDirection: TextDirection.ltr,
          )..layout();
          textPainter.paint(
            canvas,
            Offset(
              pixelPosition - textPainter.width / 2,
              currentTickLength + labelOffset,
            ),
          );
        }
      } else {
        canvas.drawLine(
          Offset(0, pixelPosition),
          Offset(currentTickLength, pixelPosition),
          currentTickPaint,
        );
        if (isMajorTick ||
            (showBoundaryLabels && (value == minValue || value == maxValue))) {
          final TextPainter textPainter = TextPainter(
            text: TextSpan(text: formatLabel(value), style: labelStyle),
            textDirection: TextDirection.ltr,
          )..layout();
          textPainter.paint(
            canvas,
            Offset(
              currentTickLength + labelOffset,
              pixelPosition - textPainter.height / 2,
            ),
          );
        }
      }
    }
  }

  @override
  bool shouldRepaint(_RulerPainter oldDelegate) {
    return oldDelegate.minValue != minValue ||
        oldDelegate.maxValue != maxValue ||
        oldDelegate.majorTickInterval != majorTickInterval ||
        oldDelegate.direction != direction ||
        oldDelegate.majorTickColor != majorTickColor ||
        oldDelegate.minorTickColor != minorTickColor ||
        oldDelegate.majorTickWidth != majorTickWidth ||
        oldDelegate.minorTickWidth != minorTickWidth ||
        oldDelegate.majorTickLength != majorTickLength ||
        oldDelegate.minorTickLength != minorTickLength ||
        oldDelegate.labelStyle != labelStyle ||
        oldDelegate.unitSpacing != unitSpacing ||
        oldDelegate.viewportDimension != viewportDimension ||
        oldDelegate.labelFormatter != labelFormatter ||
        oldDelegate.showBoundaryLabels != showBoundaryLabels ||
        oldDelegate.labelOffset != labelOffset ||
        oldDelegate.tickStrokeCap != tickStrokeCap ||
        oldDelegate.currentValue != currentValue ||
        oldDelegate.selectedTickColor != selectedTickColor ||
        oldDelegate.selectedTickWidth != selectedTickWidth ||
        oldDelegate.selectedTickLength != selectedTickLength ||
        oldDelegate.step != step;
  }

  int _getDecimalPlacesForLabels(double value) {
    if (value == value.toInt().toDouble()) return 0;
    final String valueString = value.toString();
    final int decimalPointIndex = valueString.indexOf('.');
    if (decimalPointIndex == -1) return 0;
    return math.min(valueString.length - decimalPointIndex - 1, 2);
  }
}

class _ActiveRangePainter extends CustomPainter {
  final Color color;
  final Axis direction;
  final double highlightThickness;

  _ActiveRangePainter({
    required this.color,
    required this.direction,
    required this.highlightThickness,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final Paint paint = Paint()..color = color;
    if (direction == Axis.horizontal) {
      final double centerX = size.width / 2;
      final double halfWidth = highlightThickness / 2;
      canvas.drawRect(
        Rect.fromLTRB(
            centerX - halfWidth, 0, centerX + halfWidth, size.height),
        paint,
      );
    } else {
      final double centerY = size.height / 2;
      final double halfHeight = highlightThickness / 2;
      canvas.drawRect(
        Rect.fromLTRB(
            0, centerY - halfHeight, size.width, centerY + halfHeight),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(_ActiveRangePainter oldDelegate) {
    return oldDelegate.color != color ||
        oldDelegate.direction != direction ||
        oldDelegate.highlightThickness != highlightThickness;
  }
}
