import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/constants/style_constants.dart';

enum _RegionAreaSortMode {
  area,
  coverage,
}

class AchievementRegionAreaListItem {
  const AchievementRegionAreaListItem({
    required this.name,
    required this.visitedKm2,
    required this.totalKm2,
    this.flagCountryCode,
    String? sortKey,
  }) : sortKey = sortKey ?? name;

  final String name;
  final double visitedKm2;
  final double totalKm2;
  final String? flagCountryCode;
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
  });

  final String title;
  final String emptyText;
  final List<AchievementRegionAreaListItem> items;

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
      backgroundColor: const Color(0xFF0D0D0F),
      appBar: CapsuleStyleAppBar(
        title: widget.title,
      ),
      body: SafeAreaWrapper(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(
            0,
            12,
            0,
            StyleConstants.navBarSafeArea,
          ),
          children: [
            _RegionAreaSortControl(
              value: _sortMode,
              onChanged: (value) => setState(() => _sortMode = value),
            ),
            const SizedBox(height: 12),
            if (items.isEmpty)
              _RegionAreaListEmptyCard(text: widget.emptyText)
            else
              OptionCard(
                children: [
                  for (final item in items) _RegionAreaListTile(item: item),
                ],
              ),
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
  const _RegionAreaSortControl({
    required this.value,
    required this.onChanged,
  });

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
            label: Text(context.tr('achievement.region_area_list.sort_area')),
          ),
          ButtonSegment(
            value: _RegionAreaSortMode.coverage,
            label: Text(
              context.tr('achievement.region_area_list.sort_coverage'),
            ),
          ),
        ],
        selected: {value},
        onSelectionChanged: (selected) => onChanged(selected.first),
        style: ButtonStyle(
          backgroundColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.selected)) {
              return StyleConstants.defaultColor.withValues(alpha: 0.18);
            }
            return Colors.white.withValues(alpha: 0.045);
          }),
          foregroundColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.selected)) {
              return StyleConstants.defaultColor;
            }
            return Colors.white.withValues(alpha: 0.62);
          }),
          side: WidgetStatePropertyAll(
            BorderSide(
              color: Colors.white.withValues(alpha: 0.10),
            ),
          ),
          textStyle: const WidgetStatePropertyAll(
            TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w700,
            ),
          ),
          visualDensity: VisualDensity.compact,
        ),
      ),
    );
  }
}

class _RegionAreaListTile extends StatelessWidget {
  const _RegionAreaListTile({required this.item});

  final AchievementRegionAreaListItem item;

  @override
  Widget build(BuildContext context) {
    final area = formatArea(context, item.visitedKm2);

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Container(
            width: 42,
            height: 42,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: Colors.white.withValues(alpha: 0.06),
              border: Border.all(
                color: Colors.white.withValues(alpha: 0.10),
              ),
            ),
            child: item.flagCountryCode == null
                ? Icon(
                    Icons.public_rounded,
                    color: Colors.white.withValues(alpha: 0.68),
                    size: 23,
                  )
                : AchievementCountryFlag(
                    countryCode: item.flagCountryCode!,
                    size: 34,
                  ),
          ),
          const SizedBox(width: 12),
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
                        style: const TextStyle(
                          color: Colors.white,
                          fontSize: 16,
                          fontWeight: FontWeight.w700,
                          height: 1.1,
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
                        accent: StyleConstants.defaultColor,
                        height: 6,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Text(
                      formatPercent(item.progress, fractionDigits: 3),
                      style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.62),
                        fontSize: 12,
                        fontWeight: FontWeight.w700,
                        height: 1,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
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
            style: const TextStyle(
              color: Colors.white,
              fontSize: 16,
              fontWeight: FontWeight.w800,
              height: 1,
            ),
          ),
          const SizedBox(width: 4),
          Padding(
            padding: const EdgeInsets.only(bottom: 1),
            child: Text(
              area.unit,
              style: TextStyle(
                color: Colors.white.withValues(alpha: 0.58),
                fontSize: 11,
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
            style: TextStyle(
              color: Colors.white.withValues(alpha: 0.46),
              fontSize: 13,
              fontWeight: FontWeight.w500,
              height: 1.35,
            ),
          ),
        ),
      ],
    );
  }
}
