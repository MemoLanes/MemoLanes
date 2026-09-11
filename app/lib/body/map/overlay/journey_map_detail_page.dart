import 'dart:math' as math;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_export.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart' show JourneyInfo;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'journey_detail_card.dart';

class JourneyMapDetailPage extends StatefulWidget {
  const JourneyMapDetailPage({
    super.key,
    required this.journey,
    required this.mapRendererProxy,
    required this.initialMapBounds,
  });

  final JourneyHeader journey;
  final api.MapRendererProxy mapRendererProxy;
  final MapBounds? initialMapBounds;

  @override
  State<JourneyMapDetailPage> createState() => _JourneyMapDetailPageState();
}

class _JourneyMapDetailPageState extends State<JourneyMapDetailPage> {
  late JourneyHeader _journey;
  late api.MapRendererProxy _mapRendererProxy;
  MapBounds? _mapBounds;
  bool _isEditingInformation = false;
  int _mapRevision = 0;

  @override
  void initState() {
    super.initState();
    _journey = widget.journey;
    _mapRendererProxy = widget.mapRendererProxy;
    _mapBounds = widget.initialMapBounds;
  }

  Future<void> _refreshJourney() async {
    final journeyId = _journey.id;
    final allJourneys = await api.listAllJourneys();
    JourneyHeader? latest;
    for (final journey in allJourneys) {
      if (journey.id == journeyId) {
        latest = journey;
        break;
      }
    }
    if (!mounted) return;
    if (latest == null) {
      Navigator.of(context).pop();
      return;
    }
    final latestJourney = latest;

    final rendererAndBounds = await api.getMapRendererProxyForJourney(
      journeyId: journeyId,
    );
    if (!mounted) return;
    setState(() {
      _journey = latestJourney;
      _mapRendererProxy = rendererAndBounds.$1;
      _mapBounds = rendererAndBounds.$2;
      _mapRevision++;
    });
  }

  Future<void> _saveJourneyInformation(JourneyInfo journeyInfo) async {
    await api.updateJourneyMetadata(id: _journey.id, journeyInfo: journeyInfo);
  }

  Future<void> _deleteJourney() async {
    final shouldDelete = await showCommonDialog(
      context,
      context.tr('journey.delete_journey_message'),
      hasCancel: true,
      title: context.tr('journey.delete_journey_title'),
      confirmButtonText: context.tr('common.delete'),
      confirmVariant: AppButtonVariant.danger,
    );
    if (!shouldDelete) return;
    await api.deleteJourney(journeyId: _journey.id);
    if (!mounted) return;
    Navigator.of(context).pop();
  }

  Future<void> _exportJourney() async {
    await showJourneyExportPicker(context, _journey);
  }

  Future<void> _openTrackEditor() async {
    final session = await EditSession.newInstance(journeyId: _journey.id);
    if (!mounted) return;
    if (session == null) {
      await showCommonDialog(
        context,
        context.tr('journey.editor.bitmap_not_supported'),
      );
      return;
    }
    await navigatorPush<bool>(
      context,
      page: JourneyTrackEditPage(editSession: session),
    );
    if (!mounted) return;
    await _refreshJourney();
  }

  Future<void> _showEditChoice() async {
    final choice = await showDialog<_JourneyEditChoice>(
      context: context,
      barrierColor: StyleConstants.shadowColor.withValues(
        alpha: StyleConstants.isDarkMode ? 0.58 : 0.2,
      ),
      builder: (dialogContext) => PointerInterceptor(
        child: _JourneyEditChoiceDialog(
          onSelected: (choice) => Navigator.of(dialogContext).pop(choice),
        ),
      ),
    );
    if (!mounted || choice == null) return;
    switch (choice) {
      case _JourneyEditChoice.information:
        setState(() => _isEditingInformation = true);
        break;
      case _JourneyEditChoice.track:
        await _openTrackEditor();
        break;
    }
  }

  @override
  Widget build(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final viewPadding = mediaQuery.viewPadding;
    final isLandscape = mediaQuery.orientation == Orientation.landscape;
    final detailCardPadding = isLandscape ? 190.0 : 330.0;

    return Scaffold(
      backgroundColor: StyleConstants.canvasColor,
      body: Stack(
        fit: StackFit.expand,
        children: [
          BaseMapWebview(
            key: ValueKey('journey-detail-${_journey.id}-$_mapRevision'),
            mapRendererProxy: _mapRendererProxy,
            initialMapBounds: _mapBounds,
            initialMapBoundsPadding:
                CapsuleStyleOverlayAppBar.mapFitPaddingForBottomOverlay(
                  context,
                  edgePadding: 28,
                  bottomOverlayHeight: detailCardPadding + viewPadding.bottom,
                ),
          ),
          Positioned(
            left: viewPadding.left + 16,
            right: viewPadding.right + 16,
            bottom: viewPadding.bottom + 16,
            child: Align(
              alignment: Alignment.bottomCenter,
              child: SizedBox(
                width: math.min(
                  mediaQuery.size.width -
                      viewPadding.left -
                      viewPadding.right -
                      32,
                  430.0,
                ),
                child: PointerInterceptor(
                  child: CollapsibleJourneyDetail(
                    child: JourneyDetailCard(
                      journey: _journey,
                      isEditing: _isEditingInformation,
                      onExport: _exportJourney,
                      onEdit: _showEditChoice,
                      onDelete: _deleteJourney,
                      onSave: _saveJourneyInformation,
                      onSaved: () async {
                        await _refreshJourney();
                        if (!mounted) return;
                        setState(() => _isEditingInformation = false);
                      },
                    ),
                  ),
                ),
              ),
            ),
          ),
          Positioned(
            left: viewPadding.left + 16,
            top: viewPadding.top + 14,
            child: PointerInterceptor(
              child: MapGlassBackButton(
                onPressed: () => Navigator.of(context).maybePop(),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

enum _JourneyEditChoice { information, track }

class _JourneyEditChoiceDialog extends StatelessWidget {
  const _JourneyEditChoiceDialog({required this.onSelected});

  final ValueChanged<_JourneyEditChoice> onSelected;

  @override
  Widget build(BuildContext context) {
    return Dialog(
      elevation: 0,
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.symmetric(horizontal: 38),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 360),
        child: AppDialogCard(
          title: context.tr('common.edit'),
          surfaceStyle: AppDialogSurfaceStyle.glass,
          maxHeightFactor: 0.5,
          contentPadding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              AppOptionTile(
                backgroundAlpha: 0.5,
                icon: Icons.description_outlined,
                title: context.tr('journey.journey_info_edit_page_title'),
                onTap: () => onSelected(_JourneyEditChoice.information),
              ),
              const SizedBox(height: 8),
              AppOptionTile(
                backgroundAlpha: 0.5,
                icon: Icons.edit_road_rounded,
                title: context.tr('journey.editor.page_title'),
                onTap: () => onSelected(_JourneyEditChoice.track),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
