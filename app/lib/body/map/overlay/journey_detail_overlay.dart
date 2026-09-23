import 'dart:math' as math;

import 'package:memolanes/theme/app_colors.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_export.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/body/map/journey_flow_controller.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart' show JourneyInfo;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'journey_detail_card.dart';
import 'journey_more_dialog.dart';

/// Journey details drawn above the shared map in [MapBody].
class JourneyDetailOverlay extends StatefulWidget {
  const JourneyDetailOverlay({super.key, required this.controller});

  final JourneyFlowController controller;

  @override
  State<JourneyDetailOverlay> createState() => _JourneyDetailOverlayState();
}

class _JourneyDetailOverlayState extends State<JourneyDetailOverlay> {
  JourneyFlowController get _controller => widget.controller;
  JourneyHeader get _journey => _controller.session!.journey;
  bool _moreActionInProgress = false;

  Future<void> _saveJourneyInformation(JourneyInfo journeyInfo) async {
    FocusScope.of(context).unfocus();
    try {
      await _controller.saveInformation(
        journeyInfo,
        confirm: () => showCommonDialog(
          context,
          context.tr('common.save_confirm'),
          title: context.tr('common.save'),
          hasCancel: true,
        ),
      );
    } catch (error, stackTrace) {
      log.error('Saving journey information failed: $error', stackTrace);
      if (!mounted) return;
      await showCommonDialog(
        context,
        context.tr('journey.editor.operation_failed'),
      );
    }
  }

  void _requestBack() {
    if (_controller.requestBack() != JourneyBackResult.blocked && mounted) {
      FocusScope.of(context).unfocus();
    }
  }

  Future<void> _exportJourney() async {
    if (_moreActionInProgress || _controller.phase != JourneyPhase.viewing) {
      return;
    }
    await showJourneyExportPicker(context, _journey);
  }

  Future<void> _showMore() async {
    if (_moreActionInProgress || _controller.phase != JourneyPhase.viewing) {
      return;
    }
    _moreActionInProgress = true;
    try {
      final action = await showJourneyMoreDialog(context);
      if (!mounted || action == null) return;
      switch (action) {
        case JourneyMoreAction.delete:
          await _controller.deleteJourney();
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
    await _controller.refreshTrack();
  }

  Future<void> _showEditChoice() async {
    if (_moreActionInProgress || _controller.phase != JourneyPhase.viewing) {
      return;
    }
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
        _controller.editInformation();
        break;
      case _JourneyEditChoice.track:
        try {
          await _openTrackEditor();
        } catch (error, stackTrace) {
          log.error('Loading journey editor failed: $error', stackTrace);
          if (!mounted) return;
          await showCommonDialog(
            context,
            context.tr('journey.editor.operation_failed'),
          );
        }
        break;
    }
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: _controller,
    builder: (context, _) => _buildContent(context),
  );

  Widget _buildContent(BuildContext context) {
    if (_controller.session == null) return const SizedBox.shrink();
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
                    isEditing: _controller.isEditing,
                    isSaving: _controller.phase == JourneyPhase.saving,
                    onExport: _exportJourney,
                    onEdit: _showEditChoice,
                    onMore: _showMore,
                    onCancel: _requestBack,
                    onSave: _saveJourneyInformation,
                  ),
                ),
              ),
            ),
          ),
        ),
        if (!_controller.isEditing)
          Positioned(
            left: viewPadding.left + 16,
            top: viewPadding.top + 14,
            child: PointerInterceptor(
              child: MapGlassBackButton(onPressed: _requestBack),
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
