// lib/component/map_controls/accuracy_display.dart
import 'package:flutter/material.dart';
import 'dart:ui' as ui;
import 'dart:math' as math;
import 'package:memolanes/gps_recording_state.dart';
import 'package:provider/provider.dart';

class AccuracyDisplay extends StatelessWidget {
  final bool showDebugInfo;
  final VoidCallback onToggleDebugInfo;

  const AccuracyDisplay({
    super.key,
    required this.showDebugInfo,
    required this.onToggleDebugInfo,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.all(8),
      width: 48,
      height: 48,
      child: Stack(
        alignment: Alignment.center,
        clipBehavior: Clip.none,
        children: [
          Consumer<GpsRecordingState>(
            builder: (context, gpsState, child) {
              final position = gpsState.latestPosition;
              final accuracy = position?.accuracy ?? 0.0;
              final hasData = position != null;

              return Container(
                width: 48,
                height: 48,
                decoration: BoxDecoration(
                  color: hasData ? Colors.black : Colors.black38,
                  shape: BoxShape.circle,
                ),
                child: Material(
                  color: Colors.transparent,
                  child: InkWell(
                    onTap: hasData ? onToggleDebugInfo : null,
                    borderRadius: BorderRadius.circular(24),
                    child: Stack(
                      alignment: Alignment.center,
                      children: [
                        Center(
                          child: Text(
                            hasData ? '${accuracy.round()}m\nACC' : 'NO\nGPS',
                            textAlign: TextAlign.center,
                            style: TextStyle(
                              color: hasData ? Colors.white : Colors.white60,
                              fontSize: 10,
                              height: 1.0,
                            ),
                          ),
                        ),
                        if (hasData)
                          CustomPaint(
                            size: const ui.Size(48, 48),
                            painter: AccuracyTicksPainter(
                              filledTicks: getFilledTicks(accuracy.roundToDouble()),
                              color: getAccuracyColor(accuracy),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              );
            },
          ),
          if (showDebugInfo)
            Positioned(
              right: 64,
              child: Consumer<GpsRecordingState>(
                builder: (context, gpsState, child) {
                  final position = gpsState.latestPosition;
                  if (position != null) {
                    String getSignalStatus(double accuracy) {
                      if (accuracy <= 5) return "Excellent";
                      if (accuracy <= 10) return "Good";
                      if (accuracy <= 20) return "Fair";
                      return "Poor";
                    }

                    Color getStatusColor(double accuracy) {
                      if (accuracy <= 5) return const Color(0xFFB4EC51);
                      if (accuracy <= 10) return Colors.yellow;
                      if (accuracy <= 15) return Colors.orange;
                      return Colors.red;
                    }

                    final signalStatus = getSignalStatus(position.accuracy);
                    final statusColor = getStatusColor(position.accuracy);

                    return GestureDetector(
                      onTap: onToggleDebugInfo,
                      child: Container(
                        padding: const EdgeInsets.all(16),
                        decoration: BoxDecoration(
                          color: Colors.black,
                          borderRadius: BorderRadius.circular(24),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Padding(
                                      padding:
                                          const EdgeInsets.only(right: 16.0),
                                      child: Text(
                                        '${position.accuracy.round()} m',
                                        style: const TextStyle(
                                          color: Colors.white,
                                          fontSize: 32,
                                          fontWeight: FontWeight.w400,
                                        ),
                                      ),
                                    ),
                                    const Text(
                                      'Accuracy',
                                      style: TextStyle(
                                        color: Colors.white70,
                                        fontSize: 16,
                                      ),
                                    ),
                                  ],
                                ),
                                Container(
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 8,
                                    vertical: 4,
                                  ),
                                  decoration: BoxDecoration(
                                    color: statusColor,
                                    borderRadius: BorderRadius.circular(12),
                                  ),
                                  child: Text(
                                    signalStatus,
                                    style: const TextStyle(
                                      color: Colors.black,
                                      fontSize: 12,
                                      fontWeight: FontWeight.w400,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 12),
                            Text(
                              '${position.latitude.toStringAsFixed(4)}, ${position.longitude.toStringAsFixed(4)}',
                              style: const TextStyle(
                                color: Colors.white70,
                                fontSize: 12,
                              ),
                            ),
                            Text(
                              position.timestamp
                                  .toLocal()
                                  .toString()
                                  .substring(0, 19),
                              style: const TextStyle(
                                color: Colors.white70,
                                fontSize: 12,
                              ),
                            ),
                          ],
                        ),
                      ),
                    );
                  } else {
                    return Container();
                  }
                },
              ),
            ),
        ],
      ),
    );
  }

  Color getAccuracyColor(double accuracy) {
    if (accuracy <= 5) {
      return const Color(0xFFB4EC51);
    } else if (accuracy <= 10) {
      return Colors.yellow;
    } else if (accuracy <= 20) {
      return Colors.orange;
    } else {
      return Colors.red;
    }
  }

  int getFilledTicks(double accuracy) {
    if (accuracy <= 5) {
      return 4;
    } else if (accuracy <= 10) {
      return 3;
    } else if (accuracy <= 20) {
      return 2;
    } else {
      return 1;
    }
  }
}

class AccuracyTicksPainter extends CustomPainter {
  final int filledTicks;
  final Color color;

  AccuracyTicksPainter({required this.filledTicks, required this.color});

  @override
  void paint(Canvas canvas, ui.Size size) {
    final paint = Paint()
      ..strokeWidth = 2.0
      ..style = PaintingStyle.stroke;

    final center = Offset(size.width / 2, size.height / 2);
    final radius = size.width / 2 - 1;

    const totalArcSpan = math.pi * 0.6;
    const startAngle = math.pi / 2 - totalArcSpan / 2;
    const tickArcLength = math.pi * 0.12;
    const gapAngle = (totalArcSpan - (tickArcLength * 4)) / 3;

    for (int i = 0; i < 4; i++) {
      paint.color = i < filledTicks ? color : Colors.grey.shade700;

      final tickStartAngle = startAngle + (i * (tickArcLength + gapAngle));

      canvas.drawArc(
        Rect.fromCircle(center: center, radius: radius),
        tickStartAngle,
        tickArcLength,
        false,
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant AccuracyTicksPainter oldDelegate) {
    return oldDelegate.filledTicks != filledTicks || oldDelegate.color != color;
  }
}
