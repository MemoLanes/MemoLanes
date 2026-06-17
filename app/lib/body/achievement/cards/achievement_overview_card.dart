import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/constants/index.dart';

class AchievementOverviewCard extends StatelessWidget {
  const AchievementOverviewCard({super.key});

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
              const _AreaNumber(),
              SizedBox(height: compact ? 14 : 16),
              _UnlockProgressSummary(progress: 0.01, compact: compact),
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          '探索总面积',
          style: TextStyle(
            color: Colors.white,
            fontSize: 22,
            fontWeight: FontWeight.w800,
            height: 1,
          ),
        ),
        const SizedBox(height: 8),
        Text(
          '截至 2024/05/26',
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
  const _AreaNumber();

  @override
  Widget build(BuildContext context) {
    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerLeft,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: const [
          Text(
            '128.56',
            style: TextStyle(
              color: StyleConstants.defaultColor,
              fontSize: 52,
              fontWeight: FontWeight.w900,
              letterSpacing: 0,
              height: 0.95,
            ),
          ),
          SizedBox(width: 8),
          Padding(
            padding: EdgeInsets.only(bottom: 6),
            child: Text(
              'km²',
              style: TextStyle(
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

class _UnlockProgressSummary extends StatelessWidget {
  const _UnlockProgressSummary({
    required this.progress,
    required this.compact,
  });

  final double progress;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final labelSize = 13.0;

    return Align(
      alignment: Alignment.centerLeft,
      child: IntrinsicWidth(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              padding: EdgeInsets.symmetric(
                horizontal: compact ? 14 : 16,
                vertical: compact ? 7 : 8,
              ),
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.08),
                borderRadius: BorderRadius.circular(9),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    '解锁世界面积',
                    style: TextStyle(
                      color: Colors.white.withValues(alpha: 0.64),
                      fontSize: labelSize,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  SizedBox(width: compact ? 24 : 30),
                  Text(
                    '1.3%',
                    style: TextStyle(
                      color: StyleConstants.defaultColor,
                      fontSize: labelSize,
                      fontWeight: FontWeight.w900,
                      height: 1,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 10),
            AchievementProgressLine(
              progress: progress,
              accent: StyleConstants.defaultColor,
            ),
          ],
        ),
      ),
    );
  }
}
