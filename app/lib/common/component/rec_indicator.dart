import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

// TODO(perf): Re-enable the REC pulse on Android when Flutter/WebView
// cross-Surface composition can animate without frame jitter. Other platforms,
// including iOS, retain the animation.
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
  late final Animation<double> _pulse;

  bool get _shouldAnimate => defaultTargetPlatform != TargetPlatform.android;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: Duration(milliseconds: widget.blinkDurationMs),
      vsync: this,
    );
    _pulse = CurvedAnimation(parent: _controller, curve: Curves.easeInOut);
    _syncAnimation();
  }

  @override
  void didUpdateWidget(covariant RecIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.blinkDurationMs != oldWidget.blinkDurationMs) {
      _controller.duration = Duration(milliseconds: widget.blinkDurationMs);
    }
    if (widget.isRecording != oldWidget.isRecording) {
      _syncAnimation();
    }
  }

  void _syncAnimation() {
    if (_shouldAnimate && widget.isRecording) {
      _controller.repeat(reverse: true);
    } else {
      _controller.stop();
      _controller.value = 0;
    }
  }

  Widget _buildRecordingDot({required bool animated, double pulse = 0}) {
    return SizedBox.square(
      dimension: 14,
      child: Stack(
        alignment: Alignment.center,
        children: [
          Transform.scale(
            scale: animated ? 0.8 + pulse * 0.45 : 1,
            child: SizedBox.square(
              dimension: 14,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: context.appColors.recordingColor.withValues(
                    alpha: animated ? 0.1 + pulse * 0.1 : 0.18,
                  ),
                  shape: BoxShape.circle,
                ),
              ),
            ),
          ),
          SizedBox.square(
            dimension: 7,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: context.appColors.recordingColor,
                shape: BoxShape.circle,
              ),
            ),
          ),
        ],
      ),
    );
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
                  color: context.appColors.shadowColor.withValues(
                    alpha: context.appColors.popupShadowAlpha,
                  ),
                  blurRadius: 18,
                  offset: const Offset(0, 6),
                ),
              ],
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(18),
              child: BackdropFilter(
                enabled: StyleConstants.enableBackdropFilter,
                filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 11,
                    vertical: 8,
                  ),
                  decoration: BoxDecoration(
                    color: context.appColors.glassColor.withValues(
                      alpha: lerpDouble(
                        0.72,
                        0.92,
                        context.appColors.darkProgress,
                      )!,
                    ),
                    borderRadius: BorderRadius.circular(18),
                    border: Border.all(
                      color: context.appColors.glassBorderColor.withValues(
                        alpha: lerpDouble(
                          0.86,
                          0.48,
                          context.appColors.darkProgress,
                        )!,
                      ),
                    ),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (_shouldAnimate)
                        AnimatedBuilder(
                          animation: _pulse,
                          builder: (context, _) => _buildRecordingDot(
                            animated: true,
                            pulse: _pulse.value,
                          ),
                        )
                      else
                        _buildRecordingDot(animated: false),
                      const SizedBox(width: 7),
                      Text(
                        'REC',
                        style: AppTypography.label.copyWith(
                          color: context.appColors.inkColor,
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
