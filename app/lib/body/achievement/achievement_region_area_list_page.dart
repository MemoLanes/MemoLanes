import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

enum _RegionAreaSortMode { area, coverage }

const _areaFractionDigits = 2;
const _coverageFractionDigits = 3;

class AchievementRegionAreaListItem {
  const AchievementRegionAreaListItem({
    required this.name,
    required this.visitedKm2,
    required this.totalKm2,
    this.flagCountryCode,
    this.onTap,
    String? sortKey,
  }) : sortKey = sortKey ?? name;

  final String name;
  final double visitedKm2;
  final double totalKm2;
  final String? flagCountryCode;
  final VoidCallback? onTap;
  final String sortKey;

  double get progress {
    if (totalKm2 <= 0) return 0;
    return (visitedKm2 / totalKm2).clamp(0, 1).toDouble();
  }
}

class AchievementRegionAreaListPage extends StatefulWidget {
  const AchievementRegionAreaListPage({
    super.key,
    required this.title,
    required this.emptyText,
    required this.items,
    this.isLoading = false,
    this.onRetry,
    this.showIcons = true,
    this.skeletonShowsChevron = false,
  });

  final String title;
  final String emptyText;
  final List<AchievementRegionAreaListItem> items;
  final bool isLoading;
  final VoidCallback? onRetry;
  final bool showIcons;
  final bool skeletonShowsChevron;

  @override
  State<AchievementRegionAreaListPage> createState() =>
      _AchievementRegionAreaListPageState();
}

class _AchievementRegionAreaListPageState
    extends State<AchievementRegionAreaListPage> {
  _RegionAreaSortMode _sortMode = _RegionAreaSortMode.area;

  @override
  Widget build(BuildContext context) {
    final items = _sortedItems(widget.items);

    return Scaffold(
      backgroundColor: StyleConstants.canvasColor,
      appBar: CapsuleStyleAppBar(title: widget.title),
      body: SafeAreaWrapper(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(
            0,
            12,
            0,
            StyleConstants.navBarSafeArea,
          ),
          children: [
            if (widget.isLoading)
              _RegionAreaListSkeleton(
                showIcons: widget.showIcons,
                showChevron: widget.skeletonShowsChevron,
              )
            else if (widget.onRetry != null)
              _RegionAreaListErrorCard(onRetry: widget.onRetry!)
            else if (items.isEmpty)
              _RegionAreaListEmptyCard(text: widget.emptyText)
            else ...[
              _RegionAreaSortControl(
                value: _sortMode,
                onChanged: (value) => setState(() => _sortMode = value),
              ),
              const SizedBox(height: 12),
              OptionCard(
                children: [
                  for (final item in items)
                    _RegionAreaListTile(item: item, showIcon: widget.showIcons),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }

  List<AchievementRegionAreaListItem> _sortedItems(
    List<AchievementRegionAreaListItem> items,
  ) {
    final sorted = [...items];
    sorted.sort((a, b) {
      return switch (_sortMode) {
        _RegionAreaSortMode.area => _compareWithSortKeyFallback(
          b.visitedKm2.compareTo(a.visitedKm2),
          a,
          b,
        ),
        _RegionAreaSortMode.coverage => _compareWithSortKeyFallback(
          b.progress.compareTo(a.progress),
          a,
          b,
        ),
      };
    });
    return sorted;
  }

  int _compareWithSortKeyFallback(
    int primary,
    AchievementRegionAreaListItem a,
    AchievementRegionAreaListItem b,
  ) {
    if (primary != 0) return primary;
    return a.sortKey.compareTo(b.sortKey);
  }
}

class _RegionAreaSortControl extends StatelessWidget {
  const _RegionAreaSortControl({required this.value, required this.onChanged});

  final _RegionAreaSortMode value;
  final ValueChanged<_RegionAreaSortMode> onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: SegmentedButton<_RegionAreaSortMode>(
        showSelectedIcon: false,
        segments: [
          ButtonSegment(
            value: _RegionAreaSortMode.area,
            label: Text(context.tr('achievement.region_list.sort_area')),
          ),
          ButtonSegment(
            value: _RegionAreaSortMode.coverage,
            label: Text(context.tr('achievement.region_list.sort_coverage')),
          ),
        ],
        selected: {value},
        onSelectionChanged: (selected) => onChanged(selected.first),
        style: ButtonStyle(
          backgroundColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.selected)) {
              return StyleConstants.softGreen;
            }
            return StyleConstants.surfaceColor;
          }),
          foregroundColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.selected)) {
              return StyleConstants.deepGreen;
            }
            return StyleConstants.mutedInkColor;
          }),
          side: WidgetStatePropertyAll(
            BorderSide(color: StyleConstants.lineColor),
          ),
          textStyle: const WidgetStatePropertyAll(AppTypography.label),
          visualDensity: VisualDensity.compact,
        ),
      ),
    );
  }
}

class _RegionAreaListTile extends StatelessWidget {
  const _RegionAreaListTile({required this.item, required this.showIcon});

  final AchievementRegionAreaListItem item;
  final bool showIcon;

  @override
  Widget build(BuildContext context) {
    final area = formatArea(
      context,
      item.visitedKm2,
      fractionDigits: _areaFractionDigits,
    );

    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: item.onTap,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              if (showIcon) ...[
                item.flagCountryCode == null
                    ? Container(
                        width: 42,
                        height: 42,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: StyleConstants.softGreen,
                          border: Border.all(color: StyleConstants.lineColor),
                        ),
                        child: Icon(
                          Icons.public_rounded,
                          color: StyleConstants.deepGreen,
                          size: 23,
                        ),
                      )
                    : AchievementCountryFlagBadge(
                        countryCode: item.flagCountryCode!,
                      ),
                const SizedBox(width: 12),
              ],
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(
                          child: Text(
                            item.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AppTypography.subpageTitle.copyWith(
                              color: StyleConstants.inkColor,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        const SizedBox(width: 10),
                        _RegionAreaText(area: area),
                      ],
                    ),
                    const SizedBox(height: 9),
                    Row(
                      children: [
                        Expanded(
                          child: AchievementProgressLine(
                            progress: item.progress,
                            accent: StyleConstants.primaryGreen,
                            height: 6,
                          ),
                        ),
                        const SizedBox(width: 10),
                        Text(
                          formatPercent(
                            item.progress,
                            fractionDigits: _coverageFractionDigits,
                          ),
                          style: AppTypography.label.copyWith(
                            color: StyleConstants.mutedInkColor,
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              if (item.onTap != null) ...[
                const SizedBox(width: 8),
                Icon(
                  Icons.chevron_right_rounded,
                  color: StyleConstants.mutedInkColor,
                  size: 22,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _RegionAreaListSkeleton extends StatelessWidget {
  const _RegionAreaListSkeleton({
    required this.showIcons,
    required this.showChevron,
  });

  final bool showIcons;
  final bool showChevron;

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        for (var i = 0; i < 6; i++)
          _RegionAreaListSkeletonTile(
            showIcon: showIcons,
            showChevron: showChevron,
          ),
      ],
    );
  }
}

class _RegionAreaListSkeletonTile extends StatelessWidget {
  const _RegionAreaListSkeletonTile({
    required this.showIcon,
    required this.showChevron,
  });

  final bool showIcon;
  final bool showChevron;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
      child: Row(
        children: [
          if (showIcon) ...[
            const _RegionAreaSkeletonBlock(width: 42, height: 42, radius: 999),
            const SizedBox(width: 12),
          ],
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Row(
                  children: [
                    Expanded(
                      child: Align(
                        alignment: Alignment.centerLeft,
                        child: _RegionAreaSkeletonBlock(width: 128, height: 16),
                      ),
                    ),
                    SizedBox(width: 10),
                    _RegionAreaSkeletonBlock(width: 78, height: 16),
                  ],
                ),
                const SizedBox(height: 9),
                Row(
                  children: [
                    const Expanded(
                      child: _RegionAreaSkeletonBlock(
                        width: double.infinity,
                        height: 6,
                        radius: 999,
                      ),
                    ),
                    const SizedBox(width: 10),
                    const _RegionAreaSkeletonBlock(width: 54, height: 12),
                  ],
                ),
              ],
            ),
          ),
          if (showChevron) ...[
            const SizedBox(width: 8),
            const _RegionAreaSkeletonBlock(width: 22, height: 22),
          ],
        ],
      ),
    );
  }
}

class _RegionAreaSkeletonBlock extends StatelessWidget {
  const _RegionAreaSkeletonBlock({
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
        color: StyleConstants.lineColor.withValues(alpha: 0.78),
        borderRadius: BorderRadius.circular(radius),
      ),
      child: SizedBox(width: width, height: height),
    );
  }
}

class _RegionAreaText extends StatelessWidget {
  const _RegionAreaText({required this.area});

  final FormattedArea area;

  @override
  Widget build(BuildContext context) {
    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerRight,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            area.value,
            style: AppTypography.subpageTitle.copyWith(
              color: StyleConstants.inkColor,
              fontWeight: FontWeight.w800,
            ),
          ),
          const SizedBox(width: 4),
          Padding(
            padding: const EdgeInsets.only(bottom: 1),
            child: Text(
              area.unit,
              style: AppTypography.micro.copyWith(
                color: StyleConstants.mutedInkColor,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _RegionAreaListEmptyCard extends StatelessWidget {
  const _RegionAreaListEmptyCard({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 28),
          child: Text(
            text,
            textAlign: TextAlign.center,
            style: AppTypography.supporting.copyWith(
              color: StyleConstants.mutedInkColor,
            ),
          ),
        ),
      ],
    );
  }
}

class _RegionAreaListErrorCard extends StatelessWidget {
  const _RegionAreaListErrorCard({required this.onRetry});

  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 28),
          child: Column(
            children: [
              Text(
                context.tr('achievement.region_list.error'),
                textAlign: TextAlign.center,
                style: AppTypography.supporting.copyWith(
                  color: StyleConstants.mutedInkColor,
                ),
              ),
              const SizedBox(height: 14),
              OutlinedButton.icon(
                onPressed: onRetry,
                icon: const Icon(Icons.refresh_rounded, size: 18),
                label: Text(context.tr('achievement.region_list.retry')),
                style: OutlinedButton.styleFrom(
                  foregroundColor: StyleConstants.deepGreen,
                  side: BorderSide(color: StyleConstants.primaryGreen),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
