import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/constants/index.dart';

class AchievementOverviewCard extends StatelessWidget {
  const AchievementOverviewCard({
    super.key,
    required this.stats,
  });

  final AchievementAreaStats stats;

  @override
  Widget build(BuildContext context) {
    final compact = useCompactAchievementCardLayout(context);

    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const _OverviewHeader(),
              SizedBox(height: compact ? 18 : 22),
              _AreaNumber(value: stats.totalKm2),
              // TODO: Restore world unlock progress after Rust exposes a real
              // denominator/progress value.
            ],
          ),
        ),
      ],
    );
  }
}

class _OverviewHeader extends StatelessWidget {
  const _OverviewHeader();

  @override
  Widget build(BuildContext context) {
    final today = DateTime.now();
    final date = DateFormat.yMMMd(context.locale.toString()).format(today);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.tr('achievement.overview.title'),
          style: TextStyle(
            color: Colors.white,
            fontSize: 22,
            fontWeight: FontWeight.w800,
            height: 1,
          ),
        ),
        const SizedBox(height: 8),
        Text(
          context.tr('achievement.overview.as_of', args: [date]),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: Colors.white.withValues(alpha: 0.48),
            fontSize: 14,
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}

class _AreaNumber extends StatelessWidget {
  const _AreaNumber({required this.value});

  final double value;

  @override
  Widget build(BuildContext context) {
    final area = formatArea(context, value);

    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerLeft,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            area.value,
            style: const TextStyle(
              color: StyleConstants.defaultColor,
              fontSize: 52,
              fontWeight: FontWeight.w900,
              letterSpacing: 0,
              height: 0.95,
            ),
          ),
          const SizedBox(width: 8),
          Padding(
            padding: const EdgeInsets.only(bottom: 6),
            child: Text(
              area.unit,
              style: const TextStyle(
                color: StyleConstants.defaultColor,
                fontSize: 21,
                fontWeight: FontWeight.w800,
                height: 1,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
