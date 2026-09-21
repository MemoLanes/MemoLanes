// lib/component/map_controls/accuracy_display.dart
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

class AccuracyDisplay extends StatelessWidget {
  const AccuracyDisplay({super.key});

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 44,
      height: 44,
      child: Consumer<GpsManager>(
        builder: (context, gpsState, child) {
          final position = gpsState.latestPosition;
          final accuracy = position?.accuracy ?? 0.0;
          final hasData = position != null;
          final accuracyLevel = getAccuracyLevel(accuracy);
          final button = PointerInterceptor(
            child: _AccuracyButton(
              hasData: hasData,
              accuracy: accuracy,
              accuracyLevel: accuracyLevel,
            ),
          );

          if (!hasData) return button;

          return CustomPopup(
            position: PopupPosition.left,
            horizontalOffset: -16,
            contentRadius: 24,
            barrierColor: Colors.transparent,
            contentDecoration: _popupDecoration(),
            content: PointerInterceptor(child: const _AccuracyPopupContent()),
            child: button,
          );
        },
      ),
    );
  }

  BoxDecoration _popupDecoration() {
    return BoxDecoration(
      color: StyleConstants.glassColor.withValues(
        alpha: StyleConstants.isDarkMode ? 0.94 : 0.68,
      ),
      borderRadius: BorderRadius.circular(24),
      border: Border.all(
        color: StyleConstants.glassBorderColor.withValues(
          alpha: StyleConstants.isDarkMode ? 0.48 : 0.8,
        ),
      ),
      boxShadow: [
        BoxShadow(
          color: StyleConstants.shadowColor.withValues(
            alpha: StyleConstants.isDarkMode ? 0.48 : 0.14,
          ),
          blurRadius: 22,
          offset: const Offset(0, 8),
        ),
      ],
    );
  }
}

enum AccuracyLevel { excellent, good, fair, poor }

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

String getSignalStatus(BuildContext context, AccuracyLevel accuracyLevel) {
  return switch (accuracyLevel) {
    AccuracyLevel.excellent => context.tr('home.gps_signal.excellent'),
    AccuracyLevel.good => context.tr('home.gps_signal.good'),
    AccuracyLevel.fair => context.tr('home.gps_signal.fair'),
    AccuracyLevel.poor => context.tr('home.gps_signal.poor'),
  };
}

Color getStatusColor(AccuracyLevel accuracyLevel) {
  return switch (accuracyLevel) {
    AccuracyLevel.excellent => StyleConstants.statusExcellentColor,
    AccuracyLevel.good => StyleConstants.statusGoodColor,
    AccuracyLevel.fair => StyleConstants.statusFairColor,
    AccuracyLevel.poor => StyleConstants.statusPoorColor,
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
  });

  final bool hasData;
  final double accuracy;
  final AccuracyLevel accuracyLevel;

  @override
  Widget build(BuildContext context) {
    return LiquidGlassSurface(
      circular: true,
      backgroundAlpha: hasData ? 0.36 : 0.32,
      child: SizedBox(
        width: 44,
        height: 44,
        child: Material(
          color: Colors.transparent,
          child: Stack(
            alignment: Alignment.center,
            children: [
              Center(
                child: Text(
                  hasData ? '${accuracy.round()}m\nACC' : 'NO\nGPS',
                  textAlign: TextAlign.center,
                  style: AppTypography.micro.copyWith(
                    color: StyleConstants.mutedInkColor,
                    height: 1.1,
                  ),
                ),
              ),
              if (hasData)
                CustomPaint(
                  size: const ui.Size(44, 44),
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
        final statusColor = getStatusColor(accuracyLevel);

        return GestureDetector(
          onTap: () => Navigator.of(context).pop(),
          child: Column(
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
                          '${position.accuracy.round()} m',
                          style: AppTypography.dataValue.copyWith(
                            color: StyleConstants.inkColor,
                          ),
                        ),
                      ),
                      Text(
                        context.tr('home.gps_accuracy'),
                        style: AppTypography.bodyLarge.copyWith(
                          color: StyleConstants.mutedInkColor,
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
                      getSignalStatus(context, accuracyLevel),
                      style: AppTypography.caption.copyWith(
                        color: StyleConstants.isDarkMode
                            ? StyleConstants.inverseInkColor
                            : StyleConstants.surfaceColor,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                '${position.latitude.toStringAsFixed(4)}, ${position.longitude.toStringAsFixed(4)}',
                style: AppTypography.caption.copyWith(
                  color: StyleConstants.mutedInkColor,
                ),
              ),
              Text(
                position.timestamp.toLocal().toString().substring(0, 19),
                style: AppTypography.caption.copyWith(
                  color: StyleConstants.mutedInkColor,
                ),
              ),
            ],
          ),
        );
      },
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
      paint.color = i < filledTicks
          ? color
          : StyleConstants.mutedInkColor.withValues(alpha: 0.52);

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
