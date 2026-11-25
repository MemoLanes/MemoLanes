import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/achievement/achievement_region_area_list_page.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/body/achievement/shared/achievement_region_rows.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement_api;
import 'package:memolanes/utils/nav_helper.dart';

class AchievementRegionListPage extends StatefulWidget {
  const AchievementRegionListPage.childrenOf({
    super.key,
    required this.title,
    required this.parent,
    this.worldviewId,
  });

  final String title;
  final achievement_api.GeoEntityId parent;
  final String? worldviewId;

  @override
  State<AchievementRegionListPage> createState() =>
      _AchievementRegionListPageState();
}

class _AchievementRegionListPageState extends State<AchievementRegionListPage> {
  RegionRows? _regions;
  Object? _error;

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
      final regions = await loadAchievementRegionRows(widget.parent);
      if (!mounted) return;
      setState(() => _regions = regions);
    } catch (error, stackTrace) {
      log.error('load achievement regions failed: $error', stackTrace);
      if (!mounted) return;
      setState(() => _error = error);
    }
  }

  @override
  Widget build(BuildContext context) {
    final regions = _regions;
    return AchievementRegionAreaListPage(
      title: widget.title,
      items: regions == null ? const [] : _items(regions),
      emptyText: context.tr(
        regions?.isEmpty == true
            ? 'achievement.region_list.region_unsupported'
            : 'achievement.region_list.region_empty',
      ),
      isLoading: regions == null && _error == null,
      onRetry: _error == null ? null : _load,
    );
  }

  List<AchievementRegionAreaListItem> _items(RegionRows regions) {
    final worldviewId =
        widget.worldviewId ?? WorldviewManager.instance.currentWorldview.id;
    return regions.entries
        .where((entry) => entry.value.visitedAreaM2 > BigInt.zero)
        .map((entry) {
          final entity = entry.value;
          return AchievementRegionAreaListItem.fromEntity(
            entity,
            worldviewId,
            showFlag: false,
            onTap: entity.hasChildren
                ? () => navigatorPush(
                    context,
                    page: AchievementRegionListPage.childrenOf(
                      title: entity.displayName(worldviewId),
                      parent: entry.key,
                      worldviewId: worldviewId,
                    ),
                  )
                : null,
          );
        })
        .toList(growable: false);
  }
}
