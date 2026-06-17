import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/cards/option_card.dart';

const _groundExploreColor = Color(0xFFFFB86B);
const _flightExploreColor = Color(0xFF4E8BFF);

class AchievementSourceCard extends StatelessWidget {
  const AchievementSourceCard({
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
              Text(
                context.tr('achievement.source.title'),
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 22,
                  fontWeight: FontWeight.w800,
                  height: 1,
                ),
              ),
              const SizedBox(height: 12),
              Text(
                context.tr('achievement.source.description'),
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.58),
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 18),
              _SourceCardsRow(compact: compact, stats: stats),
              const SizedBox(height: 14),
              Text(
                context.tr('achievement.source.overlap_note'),
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.5),
                  fontSize: 13,
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

class _SourceCardsRow extends StatelessWidget {
  const _SourceCardsRow({
    required this.compact,
    required this.stats,
  });

  final bool compact;
  final AchievementAreaStats stats;

  @override
  Widget build(BuildContext context) {
    final gap = compact ? 8.0 : 18.0;
    final groundArea = formatArea(context, stats.groundKm2);
    final flightArea = formatArea(context, stats.flightKm2);

    return IntrinsicHeight(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: _SourceMetricCard(
              compact: compact,
              icon: Icons.directions_walk_rounded,
              title: context.tr('achievement.source.ground'),
              value: groundArea.value,
              unit: groundArea.unit,
              percentText: formatPercent(stats.groundShare),
              progress: stats.groundShare,
              accent: _groundExploreColor,
            ),
          ),
          SizedBox(width: gap),
          _PlusDivider(compact: compact),
          SizedBox(width: gap),
          Expanded(
            child: _SourceMetricCard(
              compact: compact,
              icon: Icons.route_rounded,
              title: context.tr('achievement.source.flight'),
              value: flightArea.value,
              unit: flightArea.unit,
              percentText: formatPercent(stats.flightShare),
              progress: stats.flightShare,
              accent: _flightExploreColor,
            ),
          ),
        ],
      ),
    );
  }
}

class _SourceMetricCard extends StatelessWidget {
  const _SourceMetricCard({
    required this.icon,
    required this.title,
    required this.value,
    required this.unit,
    required this.percentText,
    required this.progress,
    required this.accent,
    required this.compact,
  });

  final IconData icon;
  final String title;
  final String value;
  final String unit;
  final String percentText;
  final double progress;
  final Color accent;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final titleSize = compact ? 14.0 : 18.0;
    final valueSize = compact ? 26.0 : 34.0;
    final unitSize = compact ? 12.0 : 16.0;

    return Container(
      padding: compact
          ? const EdgeInsets.fromLTRB(10, 12, 10, 12)
          : const EdgeInsets.fromLTRB(16, 16, 16, 14),
      decoration: BoxDecoration(
        color: accent.withValues(alpha: 0.045),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: accent.withValues(alpha: 0.14),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _MetricHeader(
            icon: icon,
            title: title,
            accent: accent,
            titleSize: titleSize,
            value: value,
            unit: unit,
            valueSize: valueSize,
            unitSize: unitSize,
            compact: compact,
          ),
          SizedBox(height: compact ? 10 : 12),
          _PercentText(percentText: percentText, accent: accent),
          SizedBox(height: compact ? 10 : 12),
          AchievementProgressLine(
            progress: progress,
            accent: accent,
            height: compact ? 6 : 8,
          ),
        ],
      ),
    );
  }
}

class _MetricHeader extends StatelessWidget {
  const _MetricHeader({
    required this.icon,
    required this.title,
    required this.accent,
    required this.titleSize,
    required this.value,
    required this.unit,
    required this.valueSize,
    required this.unitSize,
    required this.compact,
  });

  final IconData icon;
  final String title;
  final Color accent;
  final double titleSize;
  final String value;
  final String unit;
  final double valueSize;
  final double unitSize;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        _MetricIcon(icon: icon, accent: accent, compact: compact),
        SizedBox(width: compact ? 8 : 16),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: accent,
                  fontSize: titleSize,
                  fontWeight: FontWeight.w800,
                  height: 1,
                ),
              ),
              const SizedBox(height: 6),
              _MetricValue(
                value: value,
                unit: unit,
                accent: accent,
                valueSize: valueSize,
                unitSize: unitSize,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _MetricValue extends StatelessWidget {
  const _MetricValue({
    required this.value,
    required this.unit,
    required this.accent,
    required this.valueSize,
    required this.unitSize,
  });

  final String value;
  final String unit;
  final Color accent;
  final double valueSize;
  final double unitSize;

  @override
  Widget build(BuildContext context) {
    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerLeft,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            value,
            style: TextStyle(
              color: accent,
              fontSize: valueSize,
              fontWeight: FontWeight.w900,
              height: 0.9,
            ),
          ),
          const SizedBox(width: 6),
          Padding(
            padding: const EdgeInsets.only(bottom: 2),
            child: Text(
              unit,
              style: TextStyle(
                color: accent,
                fontSize: unitSize,
                fontWeight: FontWeight.w700,
                height: 1,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _PercentText extends StatelessWidget {
  const _PercentText({
    required this.percentText,
    required this.accent,
  });

  final String percentText;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    return RichText(
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      text: TextSpan(
        style: TextStyle(
          color: Colors.white.withValues(alpha: 0.64),
          fontSize: 13,
          fontWeight: FontWeight.w600,
        ),
        children: [
          TextSpan(text: context.tr('achievement.source.share_prefix')),
          TextSpan(
            text: percentText,
            style: TextStyle(
              color: accent,
              fontWeight: FontWeight.w900,
            ),
          ),
        ],
      ),
    );
  }
}

class _MetricIcon extends StatelessWidget {
  const _MetricIcon({
    required this.icon,
    required this.accent,
    this.compact = false,
  });

  final IconData icon;
  final Color accent;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final size = compact ? 44.0 : 76.0;

    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: accent.withValues(alpha: 0.08),
        border: Border.all(
          color: accent.withValues(alpha: 0.48),
          width: compact ? 2 : 3,
        ),
        boxShadow: [
          BoxShadow(
            color: accent.withValues(alpha: 0.18),
            blurRadius: 22,
            spreadRadius: -4,
          ),
        ],
      ),
      child: Icon(
        icon,
        color: accent,
        size: compact ? 24 : 38,
      ),
    );
  }
}

class _PlusDivider extends StatelessWidget {
  const _PlusDivider({required this.compact});

  final bool compact;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: compact ? 24 : 38,
      child: Stack(
        alignment: Alignment.center,
        children: [
          Positioned.fill(
            child: Center(
              child: Container(
                width: 1,
                color: Colors.white.withValues(alpha: 0.09),
              ),
            ),
          ),
          _PlusBubble(compact: compact),
        ],
      ),
    );
  }
}

class _PlusBubble extends StatelessWidget {
  const _PlusBubble({required this.compact});

  final bool compact;

  @override
  Widget build(BuildContext context) {
    final size = compact ? 22.0 : 30.0;

    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        color: const Color(0xFF17212B),
        shape: BoxShape.circle,
        border: Border.all(
          color: Colors.white.withValues(alpha: 0.08),
        ),
      ),
      child: Icon(
        Icons.add_rounded,
        color: Colors.white.withValues(alpha: 0.72),
        size: compact ? 16 : 21,
      ),
    );
  }
}
