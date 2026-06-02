import 'package:flutter/material.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

class MapPanelPage extends StatelessWidget {
  const MapPanelPage({
    super.key,
    required this.title,
    required this.mapRendererProxy,
    required this.panel,
    this.initialMapView,
    this.maxHeight = 510,
    this.expandPanel = false,
    this.loadingBody = const SizedBox.shrink(),
    this.onBack,
  });

  final String title;
  final api.MapRendererProxy? mapRendererProxy;
  final MapView? initialMapView;
  final Widget panel;
  final double maxHeight;
  final bool expandPanel;
  final Widget loadingBody;
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = this.mapRendererProxy;

    return Scaffold(
      body: Stack(
        children: [
          SlidingUpPanel(
            color: Colors.black,
            borderRadius: const BorderRadius.only(
              topLeft: Radius.circular(16.0),
              topRight: Radius.circular(16.0),
            ),
            maxHeight: maxHeight,
            defaultPanelState: PanelState.OPEN,
            panel: PointerInterceptor(
              child: Center(
                child: Column(
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(top: 12.0),
                      child: CustomPaint(
                        size: const Size(40.0, 4.0),
                        painter: LinePainter(color: const Color(0xFFB5B5B5)),
                      ),
                    ),
                    const SizedBox(height: 16.0),
                    if (expandPanel) Expanded(child: panel) else panel,
                  ],
                ),
              ),
            ),
            body: mapRendererProxy == null
                ? loadingBody
                : BaseMapWebview(
                    key: const ValueKey("mapWidget"),
                    mapRendererProxy: mapRendererProxy,
                    initialMapView: initialMapView,
                  ),
          ),
          CapsuleStyleOverlayAppBar.overlayBar(
            title: title,
            onBack: onBack,
          ),
        ],
      ),
    );
  }
}
