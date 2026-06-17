import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/cards/achievement_coming_soon_card.dart';
import 'package:memolanes/body/achievement/cards/achievement_overview_card.dart';
import 'package:memolanes/body/achievement/cards/achievement_source_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:provider/provider.dart';

class AchievementBody extends StatelessWidget {
  const AchievementBody({super.key});

  @override
  Widget build(BuildContext context) {
    final hasOngoingJourney =
        context.watch<GpsManager>().recordingStatus != GpsRecordingStatus.none;

    return MlSingleChildScrollView(
      padding: const EdgeInsets.symmetric(vertical: 16),
      children: [
        const _AchievementPageTitle(),
        const SizedBox(height: 20),
        if (hasOngoingJourney) ...[
          const _OngoingJourneyBanner(),
          const SizedBox(height: 14),
        ],
        const AchievementOverviewCard(),
        const SizedBox(height: 14),
        const AchievementSourceCard(),
        const SizedBox(height: 14),
        const AchievementComingSoonCard(),
      ],
    );
  }
}

class _AchievementPageTitle extends StatelessWidget {
  const _AchievementPageTitle();

  @override
  Widget build(BuildContext context) {
    return SafeAreaWrapper(
      child: SizedBox(
        width: double.infinity,
        child: Text(
          context.tr('achievement.title'),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          textAlign: TextAlign.left,
          style: TextStyle(
            color: Colors.white.withValues(alpha: 0.92),
            fontSize: 28,
            fontWeight: FontWeight.w700,
            height: 1,
          ),
        ),
      ),
    );
  }
}

class _OngoingJourneyBanner extends StatelessWidget {
  const _OngoingJourneyBanner();

  @override
  Widget build(BuildContext context) {
    const accent = Color(0xFFFFC857);

    return SafeAreaWrapper(
      child: Container(
        padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
        decoration: BoxDecoration(
          color: accent.withValues(alpha: 0.07),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: accent.withValues(alpha: 0.14)),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Icon(
              Icons.info_outline_rounded,
              color: accent,
              size: 19,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    '旅途进行中',
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 14,
                      fontWeight: FontWeight.w800,
                      height: 1.2,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    '当前成就与统计暂不包含进行中的旅途，结束并保存后会更新。',
                    style: TextStyle(
                      color: Colors.white.withValues(alpha: 0.58),
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                      height: 1.3,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
