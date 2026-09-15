import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

class AchievementComingSoonCard extends StatelessWidget {
  const AchievementComingSoonCard({super.key});

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Row(
            children: [
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: StyleConstants.warningSurfaceColor,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(
                    color: StyleConstants.achievementGoldColor.withValues(
                      alpha: 0.38,
                    ),
                  ),
                ),
                child: Icon(
                  Icons.auto_awesome_rounded,
                  color: StyleConstants.achievementGoldColor,
                  size: 22,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      context.tr('achievement.coming_soon_card.title'),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: AppTypography.cardTitle.copyWith(
                        color: StyleConstants.inkColor,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      context.tr('achievement.coming_soon_card.description'),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: AppTypography.caption.copyWith(
                        color: StyleConstants.mutedInkColor,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
