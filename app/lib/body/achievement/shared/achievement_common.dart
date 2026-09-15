import 'package:country_flags/country_flags.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:memolanes/src/rust/achievement/region.dart';

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
  const FormattedArea({required this.value, required this.unit});

  final String value;
  final String unit;
}

FormattedArea formatArea(
  BuildContext context,
  double km2, {
  int? fractionDigits,
}) {
  String formatValue(double value) => fractionDigits == null
      ? _formatNumberWithinDigits(value)
      : value.toStringAsFixed(fractionDigits);

  if (!km2.isFinite || km2 <= 0) {
    return FormattedArea(
      value: formatValue(0),
      unit: context.tr('achievement.area_units.square_meters'),
    );
  }

  if (km2 < 0.01) {
    return FormattedArea(
      value: formatValue(km2 * 1000000),
      unit: context.tr('achievement.area_units.square_meters'),
    );
  }

  if (km2 >= 99999.5) {
    return FormattedArea(
      value: formatValue(km2 / 10000),
      unit: context.tr('achievement.area_units.ten_thousand_square_kilometers'),
    );
  }

  return FormattedArea(
    value: formatValue(km2),
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

String formatPercent(double value, {int fractionDigits = 1}) {
  return '${(value * 100).toStringAsFixed(fractionDigits)}%';
}

bool useCompactAchievementCardLayout(BuildContext context) {
  return MediaQuery.sizeOf(context).width < 470;
}

class AchievementCountryFlag extends StatelessWidget {
  const AchievementCountryFlag({
    super.key,
    required this.countryCode,
    required this.size,
  });

  final String countryCode;
  final double size;

  @override
  Widget build(BuildContext context) {
    final flagCode = FlagCode.fromCountryCode(countryCode);
    if (flagCode == null) {
      return _FallbackCountryFlag(countryCode: countryCode, size: size);
    }

    return CountryFlag.fromCountryCode(
      countryCode,
      theme: ImageTheme(width: size, height: size, shape: const Circle()),
    );
  }
}

/// Shared gold badge used wherever an achieved country flag is displayed.
class AchievementCountryFlagBadge extends StatelessWidget {
  const AchievementCountryFlagBadge({
    super.key,
    required this.countryCode,
    this.size = 42,
    this.flagSize = 36,
  });

  final String countryCode;
  final double size;
  final double flagSize;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        gradient: LinearGradient(
          colors: [
            StyleConstants.achievementGoldSurfaceStart,
            StyleConstants.achievementGoldSurfaceEnd,
          ],
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
        ),
        boxShadow: [
          BoxShadow(
            color: StyleConstants.achievementGoldColor.withValues(alpha: 0.25),
            blurRadius: 6,
          ),
        ],
      ),
      child: AchievementCountryFlag(countryCode: countryCode, size: flagSize),
    );
  }
}

class _FallbackCountryFlag extends StatelessWidget {
  const _FallbackCountryFlag({required this.countryCode, required this.size});

  final String countryCode;
  final double size;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: StyleConstants.canvasColor,
        border: Border.all(color: StyleConstants.lineColor),
      ),
      child: Icon(
        Icons.public_rounded,
        color: StyleConstants.mutedInkColor,
        size: size * 0.54,
      ),
    );
  }
}

class AchievementCardTitleRow extends StatelessWidget {
  const AchievementCardTitleRow({
    super.key,
    required this.title,
    required this.info,
  });

  final String title;
  final String info;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      mainAxisSize: MainAxisSize.min,
      children: [
        Flexible(
          child: Text(
            title,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: AppTypography.metricTitle.copyWith(
              color: StyleConstants.inkColor,
            ),
          ),
        ),
        const SizedBox(width: 4),
        CustomPopup(
          position: PopupPosition.top,
          verticalOffset: 8,
          contentRadius: 16,
          backgroundColor: StyleConstants.surfaceColor,
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 12,
            vertical: 10,
          ),
          barrierColor: Colors.transparent,
          content: PointerInterceptor(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 260),
              child: Text(
                info,
                style: AppTypography.supporting.copyWith(
                  color: StyleConstants.inkColor,
                ),
              ),
            ),
          ),
          child: Padding(
            padding: const EdgeInsets.only(top: 1),
            child: Icon(
              Icons.info_outline_rounded,
              color: StyleConstants.mutedInkColor,
              size: 18,
            ),
          ),
        ),
      ],
    );
  }
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
            ColoredBox(color: StyleConstants.lineColor),
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
