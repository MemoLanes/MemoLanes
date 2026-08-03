// lib/component/map_controls/accuracy_display.dart
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:provider/provider.dart';

class AccuracyDisplay extends StatelessWidget {
  const AccuracyDisplay({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 8),
      width: 48,
      height: 48,
      child: Consumer<GpsManager>(
        builder: (context, gpsState, child) {
          final position = gpsState.latestPosition;
          final hasData = position != null;
          final accuracy = position?.accuracy ?? 0.0;
          final accuracyLevel = getAccuracyLevel(accuracy);
          final button = _AccuracyButton(
            hasData: hasData,
            accuracy: accuracy,
            accuracyLevel: accuracyLevel,
          );
          if (!hasData) return button;

          return CustomPopup(
            theme: CustomPopupTheme.white,
            position: PopupPosition.left,
            horizontalOffset: -16,
            contentRadius: 24,
            content: const _AccuracyPopupContent(),
            builder: (context, show) => _AccuracyButton(
              hasData: hasData,
              accuracy: accuracy,
              accuracyLevel: accuracyLevel,
              onPressed: () {
                AppHaptics.selection();
                show();
              },
            ),
          );
        },
      ),
    );
  }
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
    AccuracyLevel.excellent => StyleConstants.defaultColor,
    AccuracyLevel.good => Colors.yellow,
    AccuracyLevel.fair => Colors.orange,
    AccuracyLevel.poor => Colors.red,
  };
}

extension on AccuracyLevel {
  int get filledTicks => switch (this) {
        AccuracyLevel.excellent => 4,
        AccuracyLevel.good => 3,
        AccuracyLevel.fair || AccuracyLevel.poor => 2,
      };
}

class _AccuracyButton extends StatelessWidget {
  const _AccuracyButton({
    required this.hasData,
    required this.accuracy,
    required this.accuracyLevel,
    this.onPressed,
  });

  final bool hasData;
  final double accuracy;
  final AccuracyLevel accuracyLevel;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 48,
      height: 48,
      decoration: const BoxDecoration(
        color: StyleConstants.glassControlSurfaceColor,
        shape: BoxShape.circle,
        border: Border.fromBorderSide(
          BorderSide(color: StyleConstants.glassControlBorderColor),
        ),
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onPressed,
          customBorder: const CircleBorder(),
          child: Stack(
            alignment: Alignment.center,
            children: [
              Center(
                child: Text(
                  hasData ? '${accuracy.round()}m\nACC' : 'NO\nGPS',
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    color: hasData
                        ? StyleConstants.glassControlContentColor
                        : StyleConstants.glassControlMutedContentColor,
                    fontSize: 10,
                    height: 1.0,
                  ),
                ),
              ),
              if (hasData)
                CustomPaint(
                  size: const ui.Size(48, 48),
                  painter: AccuracyTicksPainter(
                    filledTicks: accuracyLevel.filledTicks,
                    color: getStatusColor(accuracyLevel),
                  ),
                ),
            ],
          ),
        ),
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

        final accuracyLevel = getAccuracyLevel(position.accuracy);
        return GestureDetector(
          onTap: () => Navigator.of(context).pop(),
          child: _AccuracyPopupDetails(
            accuracy: position.accuracy,
            accuracyLevel: accuracyLevel,
            location:
                '${position.latitude.toStringAsFixed(4)}, ${position.longitude.toStringAsFixed(4)}',
            timestamp: position.timestamp.toLocal().toString().substring(0, 19),
          ),
        );
      },
    );
  }
}

class _AccuracyPopupDetails extends StatelessWidget {
  const _AccuracyPopupDetails({
    required this.accuracy,
    required this.accuracyLevel,
    required this.location,
    required this.timestamp,
  });

  final double accuracy;
  final AccuracyLevel accuracyLevel;
  final String location;
  final String timestamp;

  @override
  Widget build(BuildContext context) {
    final statusColor = getStatusColor(accuracyLevel);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.only(right: 16),
                  child: Text(
                    '${accuracy.round()} m',
                    style: TextStyle(
                      color: CustomPopupTheme.white.contentColor,
                      fontSize: 32,
                      fontWeight: FontWeight.w400,
                    ),
                  ),
                ),
                Text(
                  'Accuracy',
                  style: TextStyle(
                    color: CustomPopupTheme.white.mutedContentColor,
                    fontSize: 16,
                  ),
                ),
              ],
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              decoration: BoxDecoration(
                color: statusColor,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Text(
                getSignalStatus(accuracyLevel),
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
          location,
          style: TextStyle(
            color: CustomPopupTheme.white.mutedContentColor,
            fontSize: 12,
          ),
        ),
        Text(
          timestamp,
          style: TextStyle(
            color: CustomPopupTheme.white.mutedContentColor,
            fontSize: 12,
          ),
        ),
      ],
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
