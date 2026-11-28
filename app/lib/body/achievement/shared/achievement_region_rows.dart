import 'package:memolanes/src/rust/achievement/layer.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart';

typedef RegionRows = Map<GeoEntityId, RegionEntity>;

Future<RegionRows> loadAchievementRegionRows([GeoEntityId? parent]) async {
  if (parent == null) {
    final view = await regionLevelView(
      layer: AchievementLayer.default_,
      level: RegionKind.admin0,
    );
    return view.entries;
  }
  final detail = await regionDetail(
    entityId: parent,
    layer: AchievementLayer.default_,
  );
  if (detail == null) {
    throw StateError('Achievement region is unavailable: $parent');
  }
  return detail.children;
}
