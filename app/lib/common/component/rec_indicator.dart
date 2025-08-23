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
    this.fontSize = 15.0,
    this.blinkDurationMs = 1000,
    this.borderColor = Colors.red,
    this.borderWidth = 2.0,
    this.borderRadius = 3.0,
    this.padding = const EdgeInsets.symmetric(horizontal: 3.0, vertical: 0),
    this.margin = const EdgeInsets.fromLTRB(30, 0, 0, 0),
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
          padding: widget.padding,
          decoration: BoxDecoration(
            border: Border.all(
              color: widget.borderColor,
              width: widget.borderWidth,
            ),
            borderRadius: BorderRadius.circular(widget.borderRadius),
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
                    color: Colors.red,
                    shape: BoxShape.circle,
                  ),
                ),
              ),
              SizedBox(width: widget.dotSize / 2),
              Text(
                'REC',
                style: TextStyle(
                  color: Colors.red,
                  fontWeight: FontWeight.bold,
                  fontSize: widget.fontSize,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
