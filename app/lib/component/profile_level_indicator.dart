import 'package:flutter/material.dart';
import 'dart:ui' as ui;
import 'dart:math' as math;

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
                        ? const LinearGradient(
                            begin: Alignment.topLeft,
                            end: Alignment.bottomRight,
                            colors: [
                              Color(0xFF66B6FF),
                              Color(0xFFFF99CC),
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
                      ? const Icon(
                          Icons.person,
                          color: Colors.white,
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
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: Colors.black,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Text(
                    'Lv. ${level}',
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
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

  CircularProgressPainter({
    required this.progress,
    required this.strokeWidth,
  });

  @override
  void paint(Canvas canvas, ui.Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.width - strokeWidth) / 2;

    final bgPaint = Paint()
      ..color = Colors.white.withOpacity(0.3)
      ..style = PaintingStyle.stroke
      ..strokeWidth = strokeWidth;

    canvas.drawCircle(center, radius, bgPaint);

    final progressPaint = Paint()
      ..color = const Color(0xFFB4EC51)
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
