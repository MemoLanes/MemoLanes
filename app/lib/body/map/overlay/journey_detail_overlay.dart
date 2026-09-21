import 'dart:math' as math;

import 'package:memolanes/theme/app_colors.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_export.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart' show JourneyInfo;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/src/rust/utils.dart' show MapBounds;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'journey_detail_card.dart';
import 'journey_more_dialog.dart';

/// Journey details drawn above the shared map in [MapBody].
class JourneyDetailOverlay extends StatefulWidget {
  const JourneyDetailOverlay({
    super.key,
    required this.journey,
    required this.onClose,
    required this.onMapChanged,
  });

  final JourneyHeader journey;
  final VoidCallback onClose;
  final void Function(api.MapRendererProxy proxy, MapBounds? bounds)
  onMapChanged;

  @override
  State<JourneyDetailOverlay> createState() => _JourneyDetailOverlayState();
}

class _JourneyDetailOverlayState extends State<JourneyDetailOverlay> {
  late JourneyHeader _journey;
  bool _isEditingInformation = false;
  bool _moreActionInProgress = false;
  late final VoidCallback _unregisterBackHandler;

  @override
  void initState() {
    super.initState();
    _journey = widget.journey;
    _unregisterBackHandler = registerHomeBackHandler(_handleBack);
  }

  @override
  void dispose() {
    _unregisterBackHandler();
    super.dispose();
  }

  Future<void> _refreshJourney({required bool refreshMap}) async {
    final journeyId = _journey.id;
    final latest = await api.getJourneyHeader(journeyId: journeyId);
    if (!mounted) return;
    if (latest == null) {
      widget.onClose();
      return;
    }
    if (!refreshMap) {
      // Metadata edits leave the track and viewport unchanged.
      setState(() => _journey = latest);
      return;
    }

    final rendererAndBounds = await api.getMapRendererProxyForJourney(
      journeyId: journeyId,
    );
    if (!mounted) return;
    setState(() => _journey = latest);
    widget.onMapChanged(rendererAndBounds.$1, rendererAndBounds.$2);
  }

  Future<void> _saveJourneyInformation(JourneyInfo journeyInfo) async {
    FocusScope.of(context).unfocus();
    final shouldSave = await showCommonDialog(
      context,
      context.tr('common.save_confirm'),
      title: context.tr('common.save'),
      hasCancel: true,
    );
    if (!mounted || !shouldSave) return;

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

  void _cancelJourneyInformationEdit() {
    FocusScope.of(context).unfocus();
    setState(() => _isEditingInformation = false);
  }

  void _handleBack() {
    if (_isEditingInformation) {
      _cancelJourneyInformationEdit();
    } else {
      widget.onClose();
    }
  }

  Future<void> _deleteJourney() async {
    await GlobalLoadingManager.instance.runWithLoading(
      () => api.deleteJourney(journeyId: _journey.id),
      blockNavigation: true,
    );
    if (!mounted) return;
    widget.onClose();
  }

  Future<void> _exportJourney() async {
    if (_moreActionInProgress) return;
    await showJourneyExportPicker(context, _journey);
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
      barrierColor: context.appColors.shadowColor.withValues(
        alpha: context.appColors.pickerBarrierAlpha,
      ),
      builder: (dialogContext) => _JourneyEditChoiceCard(
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
    }
  }

  @override
  Widget build(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final viewPadding = mediaQuery.viewPadding;
    return Stack(
      fit: StackFit.expand,
      children: [
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
                    onCancel: _cancelJourneyInformationEdit,
                    onSave: _saveJourneyInformation,
                  ),
                ),
              ),
            ),
          ),
        ),
        if (!_isEditingInformation)
          Positioned(
            left: viewPadding.left + 16,
            top: viewPadding.top + 14,
            child: PointerInterceptor(
              child: MapGlassBackButton(onPressed: widget.onClose),
            ),
          ),
      ],
    );
  }
}

enum _JourneyEditChoice { information, track }

class _JourneyEditChoiceCard extends StatelessWidget {
  const _JourneyEditChoiceCard({required this.onSelected});

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
        ],
      ),
    );
  }
}
