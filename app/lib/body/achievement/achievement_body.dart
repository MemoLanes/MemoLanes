import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/cards/achievement_coming_soon_card.dart';
import 'package:memolanes/body/achievement/cards/achievement_countries_card.dart';
import 'package:memolanes/body/achievement/cards/achievement_source_card.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/achievement_stats_store.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:provider/provider.dart';

class AchievementBody extends StatefulWidget {
  const AchievementBody({super.key});

  @override
  State<AchievementBody> createState() => _AchievementBodyState();
}

class _AchievementBodyState extends State<AchievementBody> {
  GpsManager? _gpsManager;
  StreamSubscription<void>? _journeyFinalizedSub;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _refreshStats();
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final gpsManager = context.read<GpsManager>();
    if (_gpsManager != gpsManager) {
      _journeyFinalizedSub?.cancel();
      _gpsManager = gpsManager;
      _journeyFinalizedSub = gpsManager.journeyFinalized.listen((_) {
        if (!mounted) return;
        _refreshStats();
      });
    }
  }

  @override
  void dispose() {
    _journeyFinalizedSub?.cancel();
    super.dispose();
  }

  void _refreshStats() {
    context.read<AchievementStatsStore>().refresh();
  }

  @override
  Widget build(BuildContext context) {
    final hasOngoingJourney =
        context.watch<GpsManager>().recordingStatus != GpsRecordingStatus.none;

    return MlSingleChildScrollView(
      padding: const EdgeInsets.only(
        top: 16,
        bottom: StyleConstants.navBarSafeArea + 16,
      ),
      children: [
        const _AchievementPageTitle(),
        const SizedBox(height: 20),
        if (hasOngoingJourney) ...[
          const _OngoingJourneyBanner(),
          const SizedBox(height: 14),
        ],
        const _AchievementStatsCards(),
        const SizedBox(height: 14),
        const AchievementCountriesCard(),
        const SizedBox(height: 14),
        const AchievementComingSoonCard(),
      ],
    );
  }
}

class _AchievementStatsCards extends StatelessWidget {
  const _AchievementStatsCards();

  @override
  Widget build(BuildContext context) {
    final store = context.watch<AchievementStatsStore>();
    final stats = store.stats;

    if (stats == null) {
      if (store.isAreaStatsLoading) {
        return const _AchievementStatsSkeleton();
      }
      if (store.areaStatsError != null) {
        return const _AchievementStatsErrorCard();
      }
      return const SizedBox.shrink();
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        AchievementSourceCard(stats: stats),
      ],
    );
  }
}

class _AchievementStatsErrorCard extends StatelessWidget {
  const _AchievementStatsErrorCard();

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              AchievementCardTitleRow(
                title: context.tr('achievement.source.title'),
                info: context.tr('achievement.source.overlap_note'),
              ),
              const SizedBox(height: 12),
              Text(
                context.tr('achievement.source.error'),
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.58),
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _AchievementStatsSkeleton extends StatelessWidget {
  const _AchievementStatsSkeleton();

  @override
  Widget build(BuildContext context) {
    return const Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _SourceSkeletonCard(),
      ],
    );
  }
}

class _SourceSkeletonCard extends StatelessWidget {
  const _SourceSkeletonCard();

  @override
  Widget build(BuildContext context) {
    final compact = useCompactAchievementCardLayout(context);

    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const _SkeletonBlock(width: 96, height: 22),
              const SizedBox(height: 12),
              const _SkeletonBlock(width: 220, height: 14),
              SizedBox(height: compact ? 16 : 18),
              _TotalAreaSkeleton(compact: compact),
              const SizedBox(height: 18),
              Row(
                children: [
                  Expanded(child: _SourceMetricSkeleton(compact: compact)),
                  SizedBox(width: compact ? 8 : 18),
                  _SkeletonBlock(width: compact ? 24 : 38, height: 30),
                  SizedBox(width: compact ? 8 : 18),
                  Expanded(child: _SourceMetricSkeleton(compact: compact)),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _TotalAreaSkeleton extends StatelessWidget {
  const _TotalAreaSkeleton({required this.compact});

  final bool compact;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: compact
          ? const EdgeInsets.fromLTRB(12, 12, 12, 14)
          : const EdgeInsets.fromLTRB(16, 14, 16, 16),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.035),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: Colors.white.withValues(alpha: 0.08)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _SkeletonBlock(width: 16, height: 16, radius: 999),
              SizedBox(width: 6),
              _SkeletonBlock(width: 86, height: 13),
            ],
          ),
          SizedBox(height: compact ? 10 : 12),
          const _SkeletonBlock(width: 186, height: 52),
        ],
      ),
    );
  }
}

class _SourceMetricSkeleton extends StatelessWidget {
  const _SourceMetricSkeleton({required this.compact});

  final bool compact;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: compact
          ? const EdgeInsets.fromLTRB(10, 12, 10, 12)
          : const EdgeInsets.fromLTRB(16, 16, 16, 14),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.035),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: Colors.white.withValues(alpha: 0.08)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              _SkeletonBlock(
                width: compact ? 44 : 76,
                height: compact ? 44 : 76,
                radius: 999,
              ),
              SizedBox(width: compact ? 8 : 16),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _SkeletonBlock(width: compact ? 58 : 72, height: 14),
                    const SizedBox(height: 8),
                    _SkeletonBlock(width: compact ? 74 : 96, height: 28),
                  ],
                ),
              ),
            ],
          ),
          SizedBox(height: compact ? 10 : 12),
          const _SkeletonBlock(width: 88, height: 13),
          SizedBox(height: compact ? 10 : 12),
          _SkeletonBlock(width: double.infinity, height: compact ? 6 : 8),
        ],
      ),
    );
  }
}

class _SkeletonBlock extends StatelessWidget {
  const _SkeletonBlock({
    required this.width,
    required this.height,
    this.radius = 6,
  });

  final double width;
  final double height;
  final double radius;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.075),
        borderRadius: BorderRadius.circular(radius),
      ),
      child: SizedBox(width: width, height: height),
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
                  Text(
                    context.tr('achievement.ongoing.title'),
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 14,
                      fontWeight: FontWeight.w800,
                      height: 1.2,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    context.tr('achievement.ongoing.description'),
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
