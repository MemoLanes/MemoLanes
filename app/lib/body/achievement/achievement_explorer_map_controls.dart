import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/map_controls/map_copyright_button.dart';
import 'package:memolanes/constants/style_constants.dart';

/// Viewing chrome uses the actual clear map pane, never camera-fit margins.
class AchievementExplorerMapControls extends StatelessWidget {
  const AchievementExplorerMapControls({
    super.key,
    required this.region,
    required this.readAttribution,
  });

  final Rect region;
  final Future<String?> Function()? readAttribution;

  @override
  Widget build(BuildContext context) {
    const layer = StyleConstants.mapControlSize;
    const attribution = MapCopyrightButton.buttonSize;
    final vertical = region.height >= layer + attribution + 24;
    final gap = math.min(12.0, math.max(0.0, (region.width - layer) / 2));
    final showLayer =
        region.width >= layer &&
        region.height >= layer &&
        (readAttribution == null ||
            vertical ||
            region.width >= layer + attribution + 36);
    return Stack(
      children: [
        if (showLayer)
          Positioned(
            left: region.right - gap - layer,
            top: region.top + math.min(12.0, (region.height - layer) / 2),
            child: PointerInterceptor(child: const LayerButton()),
          ),
        if (readAttribution != null &&
            region.width >= attribution &&
            region.height >= attribution)
          Positioned(
            left: vertical || !showLayer
                ? region.right - gap - attribution
                : region.right - gap - layer - 12 - attribution,
            top: vertical
                ? region.bottom - 12 - attribution
                : region.top + (region.height - attribution) / 2,
            child: PointerInterceptor(
              child: MapCopyrightButton(readAttribution: readAttribution!),
            ),
          ),
      ],
    );
  }
}
