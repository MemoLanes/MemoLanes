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

// The geo hierarchy API renamed these enum cases from country/province to
// admin0/admin1. Resolve by name so this UI remains compatible with both APIs.
final achievementCountryRegionKind = _regionKindNamed('admin0', 'country');
final achievementProvinceRegionKind = _regionKindNamed('admin1', 'province');

RegionKind _regionKindNamed(String name, String legacyName) {
  return RegionKind.values.singleWhere(
    (kind) => kind.name == name || kind.name == legacyName,
  );
}

class AchievementRegionListPage extends StatefulWidget {
  const AchievementRegionListPage({
    super.key,
    required this.title,
    required this.level,
    required this.emptyText,
    this.parent,
    this.showCountryFlags = false,
    this.loadRegionView,
    this.worldviewId,
  });

  final String title;
  final RegionKind level;
  final String emptyText;
  final achievement_api.GeoEntityId? parent;
  final bool showCountryFlags;
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
      showIcons: widget.level != achievementProvinceRegionKind,
      skeletonShowsChevron: widget.level == achievementCountryRegionKind,
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
        flagCountryCode: widget.showCountryFlags ? entity.isoA3Eh : null,
        sortKey: entity.isoA3Eh ?? entity.nameKey.value,
        onTap: widget.level == achievementCountryRegionKind
            ? () => navigatorPush(
                  context,
                  page: AchievementRegionListPage(
                    title: regionName,
                    level: achievementProvinceRegionKind,
                    parent: entry.key,
                    emptyText: context.tr(
                      'achievement.region_list.region_empty',
                    ),
                    worldviewId: worldviewId,
                  ),
                )
            : null,
      );
    }).toList(growable: false);
  }
}
