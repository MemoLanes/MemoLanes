import 'package:flutter/material.dart';

class LinePainter extends CustomPainter {
  late Color color;

  LinePainter({
    required this.color,
  });

  @override
  void paint(Canvas canvas, Size size) {
    var paint = Paint();
    paint.color = color;
    paint.strokeWidth = size.height;
    paint.style = PaintingStyle.fill;
    paint.strokeCap = StrokeCap.round;

    canvas.drawLine(Offset(0.0, 0.0), Offset(size.width, 0.0), paint);
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) {
    return false;
  }
}
