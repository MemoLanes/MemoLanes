// lib/component/map_controls/accuracy_display.dart
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/common/component/pressable_button.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:provider/provider.dart';

class AccuracyDisplay extends StatefulWidget {
  const AccuracyDisplay({
    super.key,
  });

  @override
  State<AccuracyDisplay> createState() => _AccuracyDisplayState();
}

enum AccuracyLevel {
  excellent,
  good,
  fair,
  poor,
}

AccuracyLevel getAccuracyLevel(double accuracy) {
  // TODO: tweak this
  if (accuracy < 10) {
    return AccuracyLevel.excellent;
  } else if (accuracy < 20) {
    return AccuracyLevel.good;
  } else if (accuracy < 50) {
    return AccuracyLevel.fair;
  } else {
    return AccuracyLevel.poor;
  }
}

String getSignalStatus(AccuracyLevel accuracyLevel) {
  return switch (accuracyLevel) {
    AccuracyLevel.excellent => "Excellent",
    AccuracyLevel.good => "Good",
    AccuracyLevel.fair => "Fair",
    AccuracyLevel.poor => "Poor",
  };
}

Color getStatusColor(AccuracyLevel accuracyLevel) {
  return switch (accuracyLevel) {
    AccuracyLevel.excellent => const Color(0xFFB4EC51),
    AccuracyLevel.good => Colors.yellow,
    AccuracyLevel.fair => Colors.orange,
    AccuracyLevel.poor => Colors.red,
  };
}

class _AccuracyDisplayState extends State<AccuracyDisplay> {
  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(top: 8, bottom: 8),
      width: 48,
      height: 48,
      child: Consumer<GpsManager>(
        builder: (context, gpsState, child) {
          final position = gpsState.latestPosition;
          final accuracy = position?.accuracy ?? 0.0;
          final hasData = position != null;
          final accuracyLevel = getAccuracyLevel(accuracy);
          final button = _AccuracyButton(
            hasData: hasData,
            accuracy: accuracy,
            accuracyLevel: accuracyLevel,
            filledTicks: getFilledTicks(accuracyLevel),
          );

          if (!hasData) return button;

          return CustomPopup(
            position: PopupPosition.left,
            horizontalOffset: -16,
            contentPadding: const EdgeInsets.all(16),
            contentDecoration: BoxDecoration(
              color: Colors.black,
              borderRadius: BorderRadius.circular(24),
            ),
            barrierColor: Colors.transparent,
            content: const _AccuracyPopupContent(),
            builder: (context, show) {
              return _AccuracyButton(
                hasData: hasData,
                accuracy: accuracy,
                accuracyLevel: accuracyLevel,
                filledTicks: getFilledTicks(accuracyLevel),
                onPressed: () {
                  AppHaptics.selection();
                  show();
                },
              );
            },
          );
        },
      ),
    );
  }

  int getFilledTicks(AccuracyLevel accuracyLevel) {
    return switch (accuracyLevel) {
      AccuracyLevel.excellent => 4,
      AccuracyLevel.good => 3,
      AccuracyLevel.fair => 2,
      AccuracyLevel.poor => 2,
    };
  }
}

class _AccuracyButton extends StatelessWidget {
  final bool hasData;
  final double accuracy;
  final AccuracyLevel accuracyLevel;
  final int filledTicks;
  final VoidCallback? onPressed;

  const _AccuracyButton({
    required this.hasData,
    required this.accuracy,
    required this.accuracyLevel,
    required this.filledTicks,
    this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return PressableButton.circle(
      backgroundColor: hasData ? Colors.black : Colors.black38,
      overlayColor: Colors.white.withValues(alpha: 0.18),
      onPressed: onPressed,
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
                filledTicks: filledTicks,
                color: getStatusColor(accuracyLevel),
              ),
            ),
        ],
      ),
    );
  }
}

class _AccuracyPopupContent extends StatelessWidget {
  const _AccuracyPopupContent();

  @override
  Widget build(BuildContext context) {
    return Consumer<GpsManager>(
      builder: (context, gpsState, child) {
        final position = gpsState.latestPosition;
        if (position == null) return const SizedBox.shrink();

        return _AccuracyPopupDetails(position: position);
      },
    );
  }
}

class _AccuracyPopupDetails extends StatelessWidget {
  final LocationData position;

  const _AccuracyPopupDetails({
    required this.position,
  });

  @override
  Widget build(BuildContext context) {
    final accuracyLevel = getAccuracyLevel(position.accuracy);
    final signalStatus = getSignalStatus(accuracyLevel);
    final statusColor = getStatusColor(accuracyLevel);

    return GestureDetector(
      onTap: () => Navigator.of(context).pop(),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.only(right: 16.0),
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
            position.timestamp.toLocal().toString().substring(0, 19),
            style: const TextStyle(
              color: Colors.white70,
              fontSize: 12,
            ),
          ),
        ],
      ),
    );
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
