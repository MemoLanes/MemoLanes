import 'package:memolanes/theme/app_colors.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
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
    final viewPadding = MediaQuery.viewPaddingOf(context);
    final mapPadding = CapsuleStyleOverlayAppBar.mapFitPaddingForBottomOverlay(
      context,
      bottomOverlayHeight: viewPadding.bottom + 246,
    );

    final page = Scaffold(
      backgroundColor: context.appColors.canvasColor,
      body: Stack(
        fit: StackFit.expand,
        children: [
          if (mapRendererProxy == null)
            const Center(child: CircularProgressIndicator())
          else
            BaseMapWebview(
              key: const ValueKey('importJourneyPreviewMap'),
              mapRendererProxy: mapRendererProxy,
              initialMapBounds: _initialMapBounds,
              initialMapBoundsPadding: mapPadding,
            ),
          Positioned(
            left: viewPadding.left + 16,
            right: viewPadding.right + 16,
            bottom: viewPadding.bottom + 16,
            child: Align(
              alignment: Alignment.bottomCenter,
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 430),
                child: PointerInterceptor(
                  child: ReadOnlyJourneyInfoCard(journey: widget.journeyHeader),
                ),
              ),
            ),
          ),
          Positioned(
            left: viewPadding.left + 16,
            top: viewPadding.top + 14,
            child: PointerInterceptor(
              child: MapGlassBackButton(
                onPressed: () => Navigator.maybePop(context),
              ),
            ),
          ),
        ],
      ),
    );

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: AppTheme.mapSystemOverlayStyle(Theme.of(context)),
      child: page,
    );
  }
}
