import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

class RecIndicator extends StatefulWidget {
  const RecIndicator({
    super.key,
    required this.isRecording,
    this.blinkDurationMs = 1300,
    this.margin = const EdgeInsets.fromLTRB(18, 8, 0, 0),
  });

  final bool isRecording;
  final int blinkDurationMs;
  final EdgeInsetsGeometry margin;

  @override
  State<RecIndicator> createState() => _RecIndicatorState();
}

class _RecIndicatorState extends State<RecIndicator>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late Animation<double> _pulse;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: Duration(milliseconds: widget.blinkDurationMs),
      vsync: this,
    );
    _pulse = CurvedAnimation(parent: _controller, curve: Curves.easeInOut);
    if (widget.isRecording) _controller.repeat(reverse: true);
  }

  @override
  void didUpdateWidget(covariant RecIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.blinkDurationMs != oldWidget.blinkDurationMs) {
      _controller.duration = Duration(milliseconds: widget.blinkDurationMs);
    }
    if (widget.isRecording == oldWidget.isRecording) return;
    if (widget.isRecording) {
      _controller.repeat(reverse: true);
    } else {
      _controller.stop();
      _controller.value = 0;
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.isRecording) return const SizedBox.shrink();

    return IgnorePointer(
      child: SafeArea(
        child: Align(
          alignment: Alignment.topLeft,
          child: Container(
            margin: widget.margin,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(18),
              boxShadow: [
                BoxShadow(
                  color: StyleConstants.shadowColor.withValues(
                    alpha: StyleConstants.isDarkMode ? 0.42 : 0.12,
                  ),
                  blurRadius: 18,
                  offset: const Offset(0, 6),
                ),
              ],
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(18),
              child: BackdropFilter(
                filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
                child: Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 11, vertical: 8),
                  decoration: BoxDecoration(
                    color: StyleConstants.glassColor.withValues(
                      alpha: StyleConstants.isDarkMode ? 0.92 : 0.72,
                    ),
                    borderRadius: BorderRadius.circular(18),
                    border: Border.all(
                      color: StyleConstants.glassBorderColor.withValues(
                        alpha: StyleConstants.isDarkMode ? 0.48 : 0.86,
                      ),
                    ),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      AnimatedBuilder(
                        animation: _pulse,
                        builder: (context, _) => SizedBox.square(
                          dimension: 14,
                          child: Stack(
                            alignment: Alignment.center,
                            children: [
                              Transform.scale(
                                scale: 0.8 + _pulse.value * 0.45,
                                child: Container(
                                  width: 14,
                                  height: 14,
                                  decoration: BoxDecoration(
                                    color: StyleConstants.recordingColor
                                        .withValues(
                                      alpha: 0.1 + _pulse.value * 0.1,
                                    ),
                                    shape: BoxShape.circle,
                                  ),
                                ),
                              ),
                              SizedBox.square(
                                dimension: 7,
                                child: DecoratedBox(
                                  decoration: BoxDecoration(
                                    color: StyleConstants.recordingColor,
                                    shape: BoxShape.circle,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                      const SizedBox(width: 7),
                      Text(
                        'REC',
                        style: AppTypography.label.copyWith(
                          color: StyleConstants.inkColor,
                          fontWeight: FontWeight.w700,
                          letterSpacing: 0.2,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
