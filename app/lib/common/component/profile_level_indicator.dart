import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:memolanes/theme/app_colors.dart';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';

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
                              context.appColors.profileAccentStartColor,
                              context.appColors.profileAccentEndColor,
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
                          color: context.appColors.badgeForeground,
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
                  backgroundColor: context.appColors.onStrongColor.withValues(
                    alpha: 0.3,
                  ),
                  color: context.appColors.primaryActionColor,
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
                    color: context.appColors.strongBadgeBackground,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Text(
                    'Lv. $level',
                    style: AppTypography.label.copyWith(
                      color: context.appColors.onStrongColor,
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

  final Color backgroundColor;
  final Color color;

  CircularProgressPainter({
    required this.progress,
    required this.strokeWidth,
    required this.backgroundColor,
    required this.color,
  });

  @override
  void paint(Canvas canvas, ui.Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.width - strokeWidth) / 2;

    final bgPaint = Paint()
      ..color = backgroundColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = strokeWidth;

    canvas.drawCircle(center, radius, bgPaint);

    final progressPaint = Paint()
      ..color = color
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
    return oldDelegate.progress != progress ||
        oldDelegate.strokeWidth != strokeWidth ||
        oldDelegate.backgroundColor != backgroundColor ||
        oldDelegate.color != color;
  }
}
