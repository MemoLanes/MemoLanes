import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

class ProfileLevelIndicator extends StatelessWidget {
  final int level;
  final double progress; // 0.0 to 1.0
  final double size;
  final VoidCallback? onTap;
  final ImageProvider? profileImage;

  const ProfileLevelIndicator({
    super.key,
    required this.level,
    required this.progress,
    this.size = 64.0,
    this.onTap,
    this.profileImage,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: SizedBox(
        width: size,
        // Add extra height to prevent pill clipping
        height: size + 8,
        child: Stack(
          children: [
            Positioned(
              top: 0,
              child: ClipOval(
                child: Container(
                  width: size,
                  height: size,
                  decoration: BoxDecoration(
                    gradient: profileImage == null
                        ? LinearGradient(
                            begin: Alignment.topLeft,
                            end: Alignment.bottomRight,
                            colors: [
                              StyleConstants.profileAccentStartColor,
                              StyleConstants.profileAccentEndColor,
                            ],
                          )
                        : null,
                    image: profileImage != null
                        ? DecorationImage(
                            image: profileImage!,
                            fit: BoxFit.cover,
                          )
                        : null,
                  ),
                  child: profileImage == null
                      ? Icon(
                          Icons.person,
                          color: StyleConstants.isDarkMode
                              ? StyleConstants.inverseInkColor
                              : StyleConstants.surfaceColor,
                          size: 32,
                        )
                      : null,
                ),
              ),
            ),
            Positioned(
              top: 0,
              child: CustomPaint(
                size: ui.Size(size, size),
                painter: CircularProgressPainter(
                  progress: progress,
                  strokeWidth: 4.0,
                ),
              ),
            ),
            Positioned(
              bottom: 0,
              left: 0,
              right: 0,
              child: Center(
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: StyleConstants.isDarkMode
                        ? StyleConstants.inverseInkColor
                        : StyleConstants.inkColor,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Text(
                    'Lv. $level',
                    style: AppTypography.label.copyWith(
                      color: StyleConstants.onStrongColor,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class CircularProgressPainter extends CustomPainter {
  final double progress;
  final double strokeWidth;

  CircularProgressPainter({required this.progress, required this.strokeWidth});

  @override
  void paint(Canvas canvas, ui.Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.width - strokeWidth) / 2;

    final bgPaint = Paint()
      ..color = StyleConstants.onStrongColor.withValues(alpha: 0.3)
      ..style = PaintingStyle.stroke
      ..strokeWidth = strokeWidth;

    canvas.drawCircle(center, radius, bgPaint);

    final progressPaint = Paint()
      ..color = StyleConstants.primaryActionColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = strokeWidth
      ..strokeCap = StrokeCap.round;

    canvas.drawArc(
      Rect.fromCircle(center: center, radius: radius),
      math.pi / 2,
      2 * math.pi * progress,
      false,
      progressPaint,
    );
  }

  @override
  bool shouldRepaint(CircularProgressPainter oldDelegate) {
    return oldDelegate.progress != progress;
  }
}
