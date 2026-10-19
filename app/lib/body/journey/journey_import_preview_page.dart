import 'package:memolanes/theme/app_colors.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/component/back_button_target.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class JourneyImportPreviewPage extends StatefulWidget {
  const JourneyImportPreviewPage({
    super.key,
    required this.journeyHeader,
    required this.previewJourneyData,
  });

  final JourneyHeader journeyHeader;
  final api.OpaqueJourneyData previewJourneyData;

  @override
  State<JourneyImportPreviewPage> createState() =>
      _JourneyImportPreviewPageState();
}

class _JourneyImportPreviewPageState extends State<JourneyImportPreviewPage> {
  api.MapRendererProxy? _mapRendererProxy;
  MapBounds? _initialMapBounds;
  final _detailPanelKey = GlobalKey<MapOverlayPanelState>();

  @override
  void initState() {
    super.initState();
    _loadPreview();
  }

  Future<void> _loadPreview() async {
    final rendererAndBounds = await api.getMapRendererProxyForJourneyData(
      journeyData: widget.previewJourneyData,
    );
    if (!mounted) return;
    setState(() {
      _mapRendererProxy = rendererAndBounds.$1;
      _initialMapBounds = rendererAndBounds.$2;
    });
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    final page = Scaffold(
      resizeToAvoidBottomInset: false,
      backgroundColor: context.appColors.canvasColor,
      body: AppLayoutBuilder(
        builder: (context, _) => Stack(
          fit: StackFit.expand,
          children: [
            if (mapRendererProxy == null)
              const Center(child: CircularProgressIndicator())
            else
              BaseMapWebview(
                key: const ValueKey('importJourneyPreviewMap'),
                mapRendererProxy: mapRendererProxy,
                initialMapBounds: _initialMapBounds,
                overlayPanel: _detailPanelKey,
              ),
            MapOverlayPanel.detail(
              key: _detailPanelKey,
              child: PointerInterceptor(
                child: ReadOnlyJourneyInfoCard(journey: widget.journeyHeader),
              ),
            ),
            BackButtonTarget.positioned(
              onPressed: () => Navigator.maybePop(context),
              child: MapGlassBackButton(
                onPressed: () => Navigator.maybePop(context),
              ),
            ),
          ],
        ),
      ),
    );

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: AppTheme.mapSystemOverlayStyle(Theme.of(context)),
      child: page,
    );
  }
}
