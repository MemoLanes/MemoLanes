import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart';
import 'package:memolanes/src/rust/utils.dart' show MapBounds;

const regionBounds = MapBounds(west: 0, south: 0, east: 1, north: 1);
const regionPolygon =
    '{"type":"Feature","id":"1","properties":{},'
    '"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]]}}';

RegionEntity regionRow(
  String name, {
  bool children = false,
  int area = 10,
  int total = 20,
  String? iso,
}) => RegionEntity(
  kind: RegionKind.admin0,
  nameKey: RegionNameKey(value: name),
  hasChildren: children,
  isoA3Eh: iso,
  visitedAreaM2: BigInt.from(area),
  totalAreaM2: BigInt.from(total),
);

RegionGeometry regionGeometryFixture(
  GeoEntityId id, [
  Worldview worldview = Worldview.iso,
]) => RegionGeometry(
  entityId: id,
  worldview: worldview,
  bounds: regionBounds,
  featureGeojson: regionPolygon,
);

WorldviewManager regionWorldviewManager({
  Future<void> Function(Worldview)? activate,
}) => WorldviewManager.forTesting(
  readSavedWorldview: () => Worldview.iso,
  readDeviceRegion: () async => null,
  activateGeoData: activate ?? (_) async {},
  persistWorldview: (_) {},
);
