import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/achievement/achievement_region_explorer.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

/// The shell owns the native view key. Clients publish drawing/camera inputs,
/// never move the platform view into their own layout subtree.
class SharedMapController extends ChangeNotifier {
  final mapKey = GlobalKey<BaseMapWebviewState>();
  final viewportKey = GlobalKey();
  BaseMapWebview? _record;
  AchievementRegionMapState? _explorer;
  bool recordActive = true;
  bool _explorerActive = false;
  int _cameraOwner = 0;
  api.MapRendererProxy? mainRenderer;
  bool get explorerActive => _explorerActive;
  set explorerActive(bool active) {
    if (_explorerActive == active) return;
    _explorerActive = active;
    _cameraOwner++;
  }

  MapView? returnView;
  bool returnPending = false;

  Future<MapView?> readHomeView() async => returnPending
      ? returnView
      : await mapKey.currentState?.getCurrentMapView();

  void publishRecord(BaseMapWebview map) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_disposed) return;
      _record = map;
      notifyListeners();
    });
  }

  bool _disposed = false;
  void publishExplorer(AchievementRegionMapState map) {
    if (_explorer == map) return;
    _explorer = map;
    notifyListeners();
  }

  void clearExplorer() {
    returnPending = true;
    _explorer = null;
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}

class SharedMapHost extends StatelessWidget {
  const SharedMapHost({super.key, required this.controller});
  final SharedMapController controller;

  @override
  Widget build(BuildContext context) => SizedBox.expand(
    key: controller.viewportKey,
    child: ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final map = controller._record;
        if (map == null) return const SizedBox.expand();
        final explorer = controller.explorerActive;
        final target = controller._explorer;
        return BaseMapWebview(
          key: controller.mapKey,
          mapRendererProxy: explorer
              ? controller.mainRenderer ?? map.mapRendererProxy
              : map.mapRendererProxy,
          styleStore: map.styleStore,
          initialMapView: map.initialMapView,
          flyToBounds: explorer ? target?.flyToBounds : map.flyToBounds,
          flyToView: explorer ? target?.flyToView : map.flyToView,
          returnMapView: !explorer && controller.returnPending
              ? controller.returnView
              : null,
          onMapReturnCompleted: () => controller.returnPending = false,
          boundsPadding: explorer ? target?.boundsPadding : map.boundsPadding,
          cameraOwner: controller._cameraOwner,
          cameraRevision: explorer ? target?.cameraRevision ?? 0 : null,
          achievementRegionGeoJson: explorer ? target?.featureGeojson : null,
          overlayPanel: explorer ? null : map.overlayPanel,
          trackingMode: controller.recordActive
              ? map.trackingMode
              : TrackingMode.off,
          onMapAttributionChanged: (available) {
            // Credits are document lifecycle state, not a hidden-map interaction.
            map.onMapAttributionChanged?.call(available);
          },
          onMapTap: controller.recordActive && !explorer && map.onMapTap != null
              ? () {
                  if (controller.recordActive) map.onMapTap?.call();
                }
              : null,
          onMapMoved: () {
            if (controller.recordActive) {
              controller.returnView = null;
              map.onMapMoved?.call();
            }
          },
          onRoughMapViewUpdate: (view) {
            if (controller.recordActive) map.onRoughMapViewUpdate?.call(view);
          },
        );
      },
    ),
  );
}
