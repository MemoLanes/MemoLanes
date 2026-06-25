import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';

export 'package:memolanes/common/achievement_stats_store.dart'
    show AchievementAreaStats;

const achievementCardPadding = EdgeInsets.all(16);

class FormattedArea {
  const FormattedArea({
    required this.value,
    required this.unit,
  });

  final String value;
  final String unit;
}

FormattedArea formatArea(BuildContext context, double km2) {
  if (!km2.isFinite || km2 <= 0) {
    return FormattedArea(
      value: '0',
      unit: context.tr('achievement.area_units.square_meters'),
    );
  }

  if (km2 < 0.01) {
    return FormattedArea(
      value: _formatNumberWithinDigits(km2 * 1000000),
      unit: context.tr('achievement.area_units.square_meters'),
    );
  }

  if (km2 >= 99999.5) {
    return FormattedArea(
      value: _formatNumberWithinDigits(km2 / 10000),
      unit: context.tr('achievement.area_units.ten_thousand_square_kilometers'),
    );
  }

  return FormattedArea(
    value: _formatNumberWithinDigits(km2),
    unit: context.tr('achievement.area_units.square_kilometers'),
  );
}

String _formatNumberWithinDigits(double value) {
  const maxDigits = 5;

  if (value >= 99999.5) {
    return '99999+';
  }

  final integerDigits = value.truncate().toString().length;
  final fractionDigits = (maxDigits - integerDigits).clamp(0, maxDigits);
  final fixed = value.toStringAsFixed(fractionDigits);

  if (!fixed.contains('.')) {
    return fixed;
  }

  return fixed.replaceFirst(RegExp(r'\.?0+$'), '');
}

String formatPercent(double value) {
  return '${(value * 100).toStringAsFixed(1)}%';
}

bool useCompactAchievementCardLayout(BuildContext context) {
  return MediaQuery.sizeOf(context).width < 470;
}

class AchievementProgressLine extends StatelessWidget {
  const AchievementProgressLine({
    super.key,
    required this.progress,
    required this.accent,
    this.height = 8,
  });

  final double progress;
  final Color accent;
  final double height;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(999),
      child: SizedBox(
        height: height,
        child: Stack(
          fit: StackFit.expand,
          children: [
            ColoredBox(color: Colors.white.withValues(alpha: 0.08)),
            FractionallySizedBox(
              alignment: Alignment.centerLeft,
              widthFactor: progress,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: accent,
                  borderRadius: BorderRadius.circular(999),
                  boxShadow: [
                    BoxShadow(
                      color: accent.withValues(alpha: 0.45),
                      blurRadius: 10,
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
