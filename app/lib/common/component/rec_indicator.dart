import 'dart:ui';

import 'package:flutter/material.dart';

class RecIndicator extends StatefulWidget {
  final bool isRecording;
  final double dotSize;
  final double fontSize;
  final int blinkDurationMs;
  final Color borderColor;
  final double borderWidth;
  final double borderRadius;
  final EdgeInsetsGeometry padding;
  final EdgeInsetsGeometry margin;

  const RecIndicator({
    super.key,
    required this.isRecording,
    this.dotSize = 10.0,
    this.fontSize = 14.0,
    this.blinkDurationMs = 1000,
    this.borderColor = Colors.white,
    this.borderWidth = 1.0,
    this.borderRadius = 999.0,
    this.padding = const EdgeInsets.symmetric(horizontal: 10.0, vertical: 6.0),
    this.margin = const EdgeInsets.fromLTRB(24, 0, 0, 0),
  });

  @override
  State<RecIndicator> createState() => _RecIndicatorState();
}

class _RecIndicatorState extends State<RecIndicator>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _animation;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: Duration(milliseconds: widget.blinkDurationMs),
      vsync: this,
    )..repeat(reverse: true);
    _animation = Tween<double>(begin: 1.0, end: 0.0).animate(_controller);
  }

  @override
  void didUpdateWidget(covariant RecIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.isRecording != oldWidget.isRecording) {
      if (widget.isRecording) {
        _controller.repeat(reverse: true);
      } else {
        _controller.stop();
        _controller.value = 1.0;
      }
    }
    if (widget.blinkDurationMs != oldWidget.blinkDurationMs) {
      _controller.duration = Duration(milliseconds: widget.blinkDurationMs);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.isRecording) {
      return const SizedBox.shrink();
    }
    return IgnorePointer(
      child: SafeArea(
        child: Container(
          margin: widget.margin,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(widget.borderRadius),
            child: BackdropFilter(
              filter: ImageFilter.blur(sigmaX: 2, sigmaY: 2),
              child: Container(
                padding: widget.padding,
                decoration: BoxDecoration(
                  color: Colors.white.withValues(alpha: 0.8),
                  border: Border.all(
                    color: widget.borderColor.withValues(alpha: 0.4),
                    width: widget.borderWidth,
                  ),
                  borderRadius: BorderRadius.circular(widget.borderRadius),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withValues(alpha: 0.1),
                      blurRadius: 8,
                      offset: const Offset(0, 2),
                    ),
                  ],
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    FadeTransition(
                      opacity: _animation,
                      child: Container(
                        width: widget.dotSize,
                        height: widget.dotSize,
                        decoration: const BoxDecoration(
                          color: Color(0xFFFF0000),
                          shape: BoxShape.circle,
                        ),
                      ),
                    ),
                    SizedBox(width: widget.dotSize * 0.5),
                    Text(
                      'REC',
                      style: TextStyle(
                        color: const Color(0xFF222222),
                        fontWeight: FontWeight.w700,
                        fontSize: widget.fontSize,
                        height: 1,
                        letterSpacing: 0.8,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
