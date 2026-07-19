import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/src/rust/achievement/read_model/region.dart';

export 'package:memolanes/common/achievement_stats_store.dart'
    show AchievementAreaStats;

const achievementCardPadding = EdgeInsets.all(16);

extension RegionEntityDisplay on RegionEntity {
  /// This region's localized display name for [worldviewId].
  ///
  /// This is the one sanctioned place that unwraps `nameKey.value`: [RegionNameKey]
  /// is not a `String`, so `entity.nameKey.tr()` won't compile — a raw `.tr()`
  /// resolves the worldview-agnostic name and would silently skip a
  /// worldview-specific override (`<worldview>.<name_key>`, e.g. a disputed
  /// admin-1 name).
  String displayName(String worldviewId) {
    // Unwrap to the raw `.tr()` key here and nowhere else — the `RegionNameKey`
    // wrapper exists to make `entity.nameKey.tr()` uncompilable so callers can't
    // skip the worldview override; this method is the sanctioned exception.
    final key = nameKey.value;
    String? at(String k) => trExists(k) ? k.tr() : null;
    // Worldview-scoped override first, then the worldview-agnostic name, then the
    // ISO code, then the raw key — never blank.
    return at('$worldviewId.$key') ?? at(key) ?? isoA3Eh ?? key;
  }
}

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
