import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/achievement/achievement_region_area_list_page.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/src/rust/achievement/layer.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement_api;
import 'package:memolanes/utils/nav_helper.dart';

class AchievementRegionListPage extends StatefulWidget {
  const AchievementRegionListPage.countries({
    super.key,
    required this.title,
    required this.emptyText,
  }) : parent = null,
       worldviewId = null;

  const AchievementRegionListPage.childrenOf({
    super.key,
    required this.title,
    required this.emptyText,
    required achievement_api.GeoEntityId this.parent,
    this.worldviewId,
  });

  final String title;
  final String emptyText;
  final achievement_api.GeoEntityId? parent;
  final String? worldviewId;

  @override
  State<AchievementRegionListPage> createState() =>
      _AchievementRegionListPageState();
}

class _AchievementRegionListPageState extends State<AchievementRegionListPage> {
  Map<achievement_api.GeoEntityId, RegionEntity>? _regions;
  Object? _error;

  bool get _isCountryList => widget.parent == null;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _regions = null;
      _error = null;
    });

    try {
      final regions = await _fetchRegions();
      if (!mounted) return;
      setState(() => _regions = regions);
    } catch (error, stackTrace) {
      log.error('load achievement regions failed: $error', stackTrace);
      if (!mounted) return;
      setState(() => _error = error);
    }
  }

  Future<Map<achievement_api.GeoEntityId, RegionEntity>> _fetchRegions() async {
    final parent = widget.parent;
    if (parent == null) {
      final view = await achievement_api.regionLevelView(
        layer: AchievementLayer.default_,
        level: RegionKind.admin0,
      );
      return view.entries;
    }
    final detail = await achievement_api.regionDetail(
      entityId: parent,
      layer: AchievementLayer.default_,
    );
    if (detail == null) {
      throw StateError('Achievement region is unavailable: $parent');
    }
    return detail.children;
  }

  @override
  Widget build(BuildContext context) {
    final regions = _regions;
    return AchievementRegionAreaListPage(
      title: widget.title,
      items: regions == null ? const [] : _items(regions),
      emptyText: !_isCountryList && regions?.isEmpty == true
          ? context.tr('achievement.region_list.region_unsupported')
          : widget.emptyText,
      isLoading: regions == null && _error == null,
      onRetry: _error == null ? null : _load,
      showIcons: _isCountryList,
      skeletonShowsChevron: _isCountryList,
    );
  }

  List<AchievementRegionAreaListItem> _items(
    Map<achievement_api.GeoEntityId, RegionEntity> regions,
  ) {
    final worldviewId =
        widget.worldviewId ?? WorldviewManager.instance.currentWorldview.id;
    return regions.entries
        .where((entry) => entry.value.visitedAreaM2 > BigInt.zero)
        .map((entry) {
          final entity = entry.value;
          final regionName = entity.displayName(worldviewId);
          return AchievementRegionAreaListItem(
            name: regionName,
            visitedKm2: entity.visitedAreaM2.toDouble() / 1000000,
            totalKm2: entity.totalAreaM2.toDouble() / 1000000,
            flagCountryCode: _isCountryList ? entity.isoA3Eh : null,
            sortKey: entity.isoA3Eh ?? entity.nameKey.value,
            onTap: entity.hasChildren
                ? () => navigatorPush(
                    context,
                    page: AchievementRegionListPage.childrenOf(
                      title: regionName,
                      parent: entry.key,
                      emptyText: context.tr(
                        'achievement.region_list.region_empty',
                      ),
                      worldviewId: worldviewId,
                    ),
                  )
                : null,
          );
        })
        .toList(growable: false);
  }
}
