import 'dart:math' as math;

import 'package:memolanes/theme/app_colors.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/src/rust/achievement/region.dart';

import 'package:memolanes/body/achievement/achievement_region_sort.dart';

const _areaFractionDigits = 2;
const _coverageFractionDigits = 3;

class AchievementRegionAreaListItem {
  const AchievementRegionAreaListItem({
    required this.name,
    required this.visitedKm2,
    required this.totalKm2,
    this.flagCountryCode,
    this.onTap,
    this.onDrill,
    this.selected = false,
    String? sortKey,
  }) : sortKey = sortKey ?? name;

  factory AchievementRegionAreaListItem.fromEntity(
    RegionEntity entity,
    String worldviewId, {
    required bool showFlag,
    VoidCallback? onTap,
    VoidCallback? onDrill,
    bool selected = false,
  }) => AchievementRegionAreaListItem(
    name: entity.displayName(worldviewId),
    visitedKm2: entity.visitedAreaM2.toDouble() / 1000000,
    totalKm2: entity.totalAreaM2.toDouble() / 1000000,
    flagCountryCode: showFlag ? entity.isoA3Eh : null,
    sortKey: entity.isoA3Eh ?? entity.nameKey.value,
    onTap: onTap,
    onDrill: onDrill,
    selected: selected,
  );

  final String name;
  final double visitedKm2;
  final double totalKm2;
  final String? flagCountryCode;
  final VoidCallback? onTap;
  final VoidCallback? onDrill;
  final bool selected;
  final String sortKey;

  FormattedArea formattedArea(BuildContext context) =>
      formatArea(context, visitedKm2, fractionDigits: _areaFractionDigits);

  String get coverageText =>
      formatPercent(progress, fractionDigits: _coverageFractionDigits);

  double get progress {
    if (totalKm2 <= 0) return 0;
    return (visitedKm2 / totalKm2).clamp(0, 1).toDouble();
  }
}

class AchievementRegionAreaSummary extends StatelessWidget {
  const AchievementRegionAreaSummary({super.key, required this.item});

  final AchievementRegionAreaListItem item;

  static const _nameStyle = AppTypography.itemTitle;
  static const _statsStyle = AppTypography.caption;
  static const _flagSize = 28.0;
  static const _padding = EdgeInsets.fromLTRB(16, 4, 16, 10);

  static double heightFor(TextScaler scaler) {
    double lineHeight(TextStyle style) =>
        (scaler.scale(style.fontSize!) * style.height!).ceilToDouble();
    return math.max(
          _flagSize,
          math.max(lineHeight(_nameStyle), lineHeight(_statsStyle)),
        ) +
        _padding.vertical;
  }

  @override
  Widget build(BuildContext context) {
    final area = item.formattedArea(context);
    final statsStyle = _statsStyle.copyWith(
      color: context.appColors.mutedInkColor,
    );
    return Padding(
      padding: _padding,
      child: LayoutBuilder(
        builder: (context, constraints) => Row(
          spacing: 8,
          children: [
            if (item.flagCountryCode case final code?)
              ExcludeSemantics(
                child: AchievementCountryFlagBadge(
                  countryCode: code,
                  size: _flagSize,
                  flagSize: 24,
                ),
              ),
            Expanded(
              child: Text(
                item.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: _nameStyle.copyWith(color: context.appColors.inkColor),
              ),
            ),
            ConstrainedBox(
              constraints: BoxConstraints(maxWidth: constraints.maxWidth * .55),
              child: FittedBox(
                fit: BoxFit.scaleDown,
                alignment: AlignmentDirectional.centerEnd,
                child: Row(
                  spacing: 6,
                  children: [
                    Text('${area.value} ${area.unit}', style: statsStyle),
                    ExcludeSemantics(child: Text('·', style: statsStyle)),
                    Text(item.coverageText, style: statsStyle),
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

class AchievementRegionAreaListPage extends StatelessWidget {
  const AchievementRegionAreaListPage({
    super.key,
    required this.title,
    required this.emptyText,
    required this.items,
    this.isLoading = false,
    this.onRetry,
  });

  final String title;
  final String emptyText;
  final List<AchievementRegionAreaListItem> items;
  final bool isLoading;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: context.appColors.canvasColor,
    appBar: CapsuleStyleAppBar(title: title),
    body: PageContentLayout(
      children: [
        AchievementRegionAreaListContent(
          items: items,
          emptyText: emptyText,
          isLoading: isLoading,
          onRetry: onRetry,
          showIcons: false,
          skeletonShowsChevron: false,
        ),
      ],
    ),
  );
}

class AchievementRegionAreaListContent extends StatefulWidget {
  const AchievementRegionAreaListContent({
    super.key,
    required this.items,
    required this.emptyText,
    required this.showIcons,
    required this.skeletonShowsChevron,
    required this.isLoading,
    this.onRetry,
    this.selectOnMap = false,
    this.sortMode,
    this.onSortChanged,
  });

  final List<AchievementRegionAreaListItem> items;
  final String emptyText;
  final bool showIcons;
  final bool skeletonShowsChevron;
  final bool isLoading;
  final VoidCallback? onRetry;
  final bool selectOnMap;
  final AchievementRegionSort? sortMode;
  final ValueChanged<AchievementRegionSort>? onSortChanged;

  @override
  State<AchievementRegionAreaListContent> createState() =>
      _AchievementRegionAreaListContentState();
}

class _AchievementRegionAreaListContentState
    extends State<AchievementRegionAreaListContent> {
  AchievementRegionSort _localSortMode = AchievementRegionSort.area;
  AchievementRegionSort get _sortMode => widget.sortMode ?? _localSortMode;

  @override
  Widget build(BuildContext context) {
    if (widget.isLoading) {
      return _RegionAreaListSkeleton(
        showIcons: widget.showIcons,
        showChevron: widget.skeletonShowsChevron,
      );
    }
    if (widget.onRetry case final retry?) {
      return _RegionAreaListErrorCard(onRetry: retry);
    }
    final items = _sortedItems(widget.items);
    if (items.isEmpty) return _RegionAreaListEmptyCard(text: widget.emptyText);
    final reserveChevronSpace = items.any(
      (item) => widget.selectOnMap ? item.onDrill != null : item.onTap != null,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _RegionAreaSortControl(
          value: _sortMode,
          onChanged:
              widget.onSortChanged ??
              (value) => setState(() => _localSortMode = value),
        ),
        const SizedBox(height: 12),
        OptionCard(
          useSafeArea: false,
          children: [
            for (final item in items)
              _RegionAreaListTile(
                item: item,
                showIcon: widget.showIcons,
                reserveChevronSpace: reserveChevronSpace,
                selectOnMap: widget.selectOnMap,
              ),
          ],
        ),
      ],
    );
  }

  List<AchievementRegionAreaListItem> _sortedItems(
    List<AchievementRegionAreaListItem> items,
  ) {
    final sorted = [...items];
    sorted.sort((a, b) {
      return switch (_sortMode) {
        AchievementRegionSort.area => compareAchievementRegionArea(
          aVisitedKm2: a.visitedKm2,
          aSortKey: a.sortKey,
          bVisitedKm2: b.visitedKm2,
          bSortKey: b.sortKey,
        ),
        AchievementRegionSort.coverage => _compareWithSortKeyFallback(
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

  final AchievementRegionSort value;
  final ValueChanged<AchievementRegionSort> onChanged;

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<AchievementRegionSort>(
      expandedInsets: EdgeInsets.zero,
      showSelectedIcon: false,
      segments: [
        ButtonSegment(
          value: AchievementRegionSort.area,
          label: Text(context.tr('achievement.region_list.sort_area')),
        ),
        ButtonSegment(
          value: AchievementRegionSort.coverage,
          label: Text(context.tr('achievement.region_list.sort_coverage')),
        ),
      ],
      selected: {value},
      onSelectionChanged: (selected) => onChanged(selected.first),
      style: ButtonStyle(
        backgroundColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.selected)) {
            return context.appColors.softGreen;
          }
          return context.appColors.surfaceColor;
        }),
        foregroundColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.selected)) {
            return context.appColors.deepGreen;
          }
          return context.appColors.mutedInkColor;
        }),
        side: WidgetStatePropertyAll(
          BorderSide(color: context.appColors.lineColor),
        ),
        textStyle: const WidgetStatePropertyAll(AppTypography.label),
        visualDensity: VisualDensity.compact,
      ),
    );
  }
}

class _RegionAreaListTile extends StatelessWidget {
  const _RegionAreaListTile({
    required this.item,
    required this.showIcon,
    required this.reserveChevronSpace,
    required this.selectOnMap,
  });

  final AchievementRegionAreaListItem item;
  final bool showIcon;
  final bool reserveChevronSpace;
  final bool selectOnMap;

  @override
  Widget build(BuildContext context) {
    final area = item.formattedArea(context);
    final coverage = Text(
      item.coverageText,
      style: AppTypography.label.copyWith(
        color: context.appColors.mutedInkColor,
      ),
    );
    final content = Padding(
      padding: EdgeInsets.fromLTRB(
        16,
        14,
        selectOnMap && reserveChevronSpace ? 0 : 16,
        14,
      ),
      child: Row(
        children: [
          if (showIcon) ...[
            item.flagCountryCode == null
                ? Container(
                    width: 42,
                    height: 42,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: context.appColors.softGreen,
                      border: Border.all(color: context.appColors.lineColor),
                    ),
                    child: Icon(
                      Icons.public_rounded,
                      color: context.appColors.deepGreen,
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
                          color: context.appColors.inkColor,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    if (selectOnMap)
                      Flexible(child: _RegionAreaText(area: area))
                    else
                      _RegionAreaText(area: area),
                  ],
                ),
                const SizedBox(height: 9),
                Row(
                  children: [
                    Expanded(
                      child: AchievementProgressLine(
                        progress: item.progress,
                        accent: context.appColors.primaryGreen,
                        height: 6,
                      ),
                    ),
                    const SizedBox(width: 10),
                    if (selectOnMap)
                      Flexible(
                        child: FittedBox(
                          fit: BoxFit.scaleDown,
                          alignment: Alignment.centerRight,
                          child: coverage,
                        ),
                      )
                    else
                      coverage,
                  ],
                ),
              ],
            ),
          ),
          if (reserveChevronSpace && !selectOnMap) ...[
            const SizedBox(width: 8),
            SizedBox(
              width: 22,
              child: item.onTap == null
                  ? null
                  : Icon(
                      Icons.chevron_right_rounded,
                      color: context.appColors.mutedInkColor,
                      size: 22,
                    ),
            ),
          ],
        ],
      ),
    );
    final row = InkWell(onTap: item.onTap, child: content);
    return Material(
      type: item.selected ? MaterialType.canvas : MaterialType.transparency,
      color: item.selected ? context.appColors.softGreen : null,
      child: selectOnMap
          ? Row(
              children: [
                Expanded(
                  child: Semantics(
                    container: true,
                    button: true,
                    selected: item.selected,
                    label: context.tr(
                      'achievement.region_explorer.select',
                      args: [item.name],
                    ),
                    child: row,
                  ),
                ),
                if (reserveChevronSpace)
                  if (item.onDrill case final drill?)
                    IconButton(
                      tooltip: context.tr(
                        'achievement.region_explorer.drill',
                        args: [item.name],
                      ),
                      onPressed: drill,
                      icon: Icon(
                        Icons.chevron_right_rounded,
                        color: context.appColors.mutedInkColor,
                      ),
                    )
                  else
                    const SizedBox(width: 48),
              ],
            )
          : row,
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
      useSafeArea: false,
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
            const AchievementSkeletonBlock(
              backgroundAlpha: 0.78,
              width: 42,
              height: 42,
              radius: 999,
            ),
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
                        child: AchievementSkeletonBlock(
                          backgroundAlpha: 0.78,
                          width: 128,
                          height: 16,
                        ),
                      ),
                    ),
                    SizedBox(width: 10),
                    AchievementSkeletonBlock(
                      backgroundAlpha: 0.78,
                      width: 78,
                      height: 16,
                    ),
                  ],
                ),
                const SizedBox(height: 9),
                Row(
                  children: [
                    const Expanded(
                      child: AchievementSkeletonBlock(
                        backgroundAlpha: 0.78,
                        width: double.infinity,
                        height: 6,
                        radius: 999,
                      ),
                    ),
                    const SizedBox(width: 10),
                    const AchievementSkeletonBlock(
                      backgroundAlpha: 0.78,
                      width: 54,
                      height: 12,
                    ),
                  ],
                ),
              ],
            ),
          ),
          if (showChevron) ...[
            const SizedBox(width: 8),
            const AchievementSkeletonBlock(
              backgroundAlpha: 0.78,
              width: 22,
              height: 22,
            ),
          ],
        ],
      ),
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
              color: context.appColors.inkColor,
              fontWeight: FontWeight.w800,
            ),
          ),
          const SizedBox(width: 4),
          Padding(
            padding: const EdgeInsets.only(bottom: 1),
            child: Text(
              area.unit,
              style: AppTypography.micro.copyWith(
                color: context.appColors.mutedInkColor,
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
      useSafeArea: false,
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 28),
          child: Text(
            text,
            textAlign: TextAlign.center,
            style: AppTypography.supporting.copyWith(
              color: context.appColors.mutedInkColor,
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
      useSafeArea: false,
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 28),
          child: Column(
            children: [
              Text(
                context.tr('achievement.region_list.error'),
                textAlign: TextAlign.center,
                style: AppTypography.supporting.copyWith(
                  color: context.appColors.mutedInkColor,
                ),
              ),
              const SizedBox(height: 14),
              OutlinedButton.icon(
                onPressed: onRetry,
                icon: const Icon(Icons.refresh_rounded, size: 18),
                label: Text(context.tr('achievement.region_list.retry')),
                style: OutlinedButton.styleFrom(
                  foregroundColor: context.appColors.deepGreen,
                  side: BorderSide(color: context.appColors.primaryGreen),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
