import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/achievement_region_area_list_page.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/src/rust/achievement/layer.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement_api;
import 'package:memolanes/utils/nav_helper.dart';

typedef AchievementRegionViewLoader = Future<RegionLevelView> Function();

class AchievementRegionListPage extends StatefulWidget {
  const AchievementRegionListPage({
    super.key,
    required this.title,
    required this.level,
    required this.emptyText,
    this.parent,
    this.showCountryFlags = false,
    this.flagCountryCode,
    this.loadRegionView,
    this.worldviewId,
  });

  final String title;
  final RegionKind level;
  final String emptyText;
  final achievement_api.GeoEntityId? parent;
  final bool showCountryFlags;
  final String? flagCountryCode;
  final AchievementRegionViewLoader? loadRegionView;
  final String? worldviewId;

  @override
  State<AchievementRegionListPage> createState() =>
      _AchievementRegionListPageState();
}

class _AchievementRegionListPageState extends State<AchievementRegionListPage> {
  RegionLevelView? _view;
  Object? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _view = null;
      _error = null;
    });

    try {
      final customLoader = widget.loadRegionView;
      final view = await (customLoader != null
          ? customLoader()
          : achievement_api.regionLevelView(
              layer: AchievementLayer.default_,
              level: widget.level,
              parent: widget.parent,
            ));
      if (!mounted) return;
      setState(() => _view = view);
    } catch (error, stackTrace) {
      log.error('load achievement regions failed: $error', stackTrace);
      if (!mounted) return;
      setState(() => _error = error);
    }
  }

  @override
  Widget build(BuildContext context) {
    final view = _view;
    return AchievementRegionAreaListPage(
      title: widget.title,
      items: view == null ? const [] : _items(view),
      emptyText: widget.emptyText,
      isLoading: view == null && _error == null,
      onRetry: _error == null ? null : _load,
      skeletonShowsChevron: widget.level == RegionKind.country,
    );
  }

  List<AchievementRegionAreaListItem> _items(RegionLevelView view) {
    final worldviewId =
        widget.worldviewId ?? WorldviewManager.instance.currentWorldview.id;
    return view.entries.entries
        .where((entry) => entry.value.visitedAreaM2 > BigInt.zero)
        .map((entry) {
      final entity = entry.value;
      final regionName = entity.displayName(worldviewId);
      return AchievementRegionAreaListItem(
        name: regionName,
        visitedKm2: entity.visitedAreaM2.toDouble() / 1000000,
        totalKm2: entity.totalAreaM2.toDouble() / 1000000,
        flagCountryCode:
            widget.showCountryFlags ? entity.isoA3Eh : widget.flagCountryCode,
        sortKey: entity.isoA3Eh ?? entity.nameKey.value,
        onTap: widget.level == RegionKind.country
            ? () => navigatorPush(
                  context,
                  page: AchievementRegionListPage(
                    title: context.tr(
                      'achievement.region_list.province_title',
                      args: [regionName],
                    ),
                    level: RegionKind.province,
                    parent: entry.key,
                    emptyText: context.tr(
                      'achievement.region_list.province_empty',
                    ),
                    flagCountryCode: entity.isoA3Eh,
                    worldviewId: worldviewId,
                  ),
                )
            : null,
      );
    }).toList(growable: false);
  }
}
