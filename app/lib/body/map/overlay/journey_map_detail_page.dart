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
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart' show JourneyInfo;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'journey_detail_card.dart';
import 'journey_more_dialog.dart';

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
  bool _moreActionInProgress = false;

  @override
  void initState() {
    super.initState();
    _journey = widget.journey;
    _mapRendererProxy = widget.mapRendererProxy;
    _mapBounds = widget.initialMapBounds;
  }

  Future<void> _refreshJourney({required bool refreshMap}) async {
    final journeyId = _journey.id;
    final latest = await api.getJourneyHeader(journeyId: journeyId);
    if (!mounted) return;
    if (latest == null) {
      popCurrentRoute(context);
      return;
    }
    if (!refreshMap) {
      // Metadata edits leave the track unchanged. Keep the WebView's key,
      // renderer and viewport so saving does not reload or recenter the map.
      setState(() => _journey = latest);
      return;
    }

    final rendererAndBounds = await api.getMapRendererProxyForJourney(
      journeyId: journeyId,
    );
    if (!mounted) return;
    setState(() {
      _journey = latest;
      _mapRendererProxy = rendererAndBounds.$1;
      _mapBounds = rendererAndBounds.$2;
      _mapRevision++;
    });
  }

  Future<void> _saveJourneyInformation(JourneyInfo journeyInfo) async {
    await GlobalLoadingManager.instance.runWithLoading(() async {
      await api.updateJourneyMetadata(
        id: _journey.id,
        journeyInfo: journeyInfo,
      );
      if (!mounted) return;
      await _refreshJourney(refreshMap: false);
    }, blockNavigation: true);
    if (!mounted) return;
    setState(() => _isEditingInformation = false);
  }

  Future<void> _deleteJourney() async {
    await GlobalLoadingManager.instance.runWithLoading(
      () => api.deleteJourney(journeyId: _journey.id),
      blockNavigation: true,
    );
    if (!mounted) return;
    popCurrentRoute(context);
  }

  Future<void> _exportJourney() async {
    if (_moreActionInProgress) return;
    await showJourneyExportPicker(
      context,
      _journey,
      hasRawData: _journey.hasRawData,
    );
  }

  Future<void> _deleteRawData() async {
    final confirmed = await showCommonDialog(
      context,
      context.tr('journey.delete_raw_data_message'),
      hasCancel: true,
      title: context.tr('journey.delete_raw_data_title'),
      confirmButtonText: context.tr('common.delete'),
      confirmVariant: AppButtonVariant.danger,
    );
    if (!confirmed || !mounted) return;

    try {
      await GlobalLoadingManager.instance.runWithLoading(
        () => api.deleteJourneyRawData(journeyId: _journey.id),
        blockNavigation: true,
      );
      if (!mounted) return;
      await _refreshJourney(refreshMap: false);
    } catch (error, stackTrace) {
      log.error('Deleting journey raw data failed: $error', stackTrace);
      if (!mounted) return;
      await showCommonDialog(
        context,
        context.tr('journey.editor.operation_failed'),
      );
    }
  }

  Future<void> _showMore() async {
    if (_moreActionInProgress) return;
    _moreActionInProgress = true;
    try {
      final action = await showJourneyMoreDialog(context);
      if (!mounted || action == null) return;
      switch (action) {
        case JourneyMoreAction.delete:
          await _deleteJourney();
          break;
      }
    } catch (error, stackTrace) {
      log.error('Journey action failed: $error', stackTrace);
      if (!mounted) return;
      await showCommonDialog(
        context,
        context.tr('journey.editor.operation_failed'),
      );
    } finally {
      _moreActionInProgress = false;
    }
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
    final saved = await navigatorPush<bool>(
      context,
      page: JourneyTrackEditPage(editSession: session),
    );
    if (!mounted || saved != true) return;
    await _refreshJourney(refreshMap: true);
  }

  Future<void> _showEditChoice() async {
    if (_moreActionInProgress) return;
    final choice = await showAppDialog<_JourneyEditChoice>(
      context,
      maxWidth: 360,
      insetPadding: const EdgeInsets.symmetric(horizontal: 38),
      barrierColor: StyleConstants.shadowColor.withValues(
        alpha: StyleConstants.isDarkMode ? 0.58 : 0.2,
      ),
      builder: (dialogContext) => _JourneyEditChoiceCard(
        hasRawData: _journey.hasRawData,
        onSelected: (choice) => Navigator.of(dialogContext).pop(choice),
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
      case _JourneyEditChoice.deleteRawData:
        await _deleteRawData();
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
            // Scaffold already removes the keyboard height from the body.
            // Bound the card between the back button and the bottom inset.
            top: viewPadding.top + 14 + 42 + 12,
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
                      onMore: _showMore,
                      onSave: _saveJourneyInformation,
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

enum _JourneyEditChoice { information, track, deleteRawData }

class _JourneyEditChoiceCard extends StatelessWidget {
  const _JourneyEditChoiceCard({
    required this.hasRawData,
    required this.onSelected,
  });

  final bool hasRawData;
  final ValueChanged<_JourneyEditChoice> onSelected;

  @override
  Widget build(BuildContext context) {
    return AppDialogCard(
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
          if (hasRawData) ...[
            const SizedBox(height: 8),
            AppOptionTile(
              backgroundAlpha: 0.5,
              iconWidget: Icon(
                Icons.delete_outline_rounded,
                color: StyleConstants.dangerInkColor,
                size: 20,
              ),
              title: context.tr('journey.delete_raw_data'),
              onTap: () => onSelected(_JourneyEditChoice.deleteRawData),
            ),
          ],
        ],
      ),
    );
  }
}
