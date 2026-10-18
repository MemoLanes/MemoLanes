import 'package:memolanes/theme/app_colors.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

import 'package:memolanes/body/journey/journey_info_form.dart';

class CollapsibleJourneyDetail extends StatefulWidget {
  const CollapsibleJourneyDetail({super.key, required this.child});

  final Widget child;

  @override
  State<CollapsibleJourneyDetail> createState() =>
      _CollapsibleJourneyDetailState();
}

class _CollapsibleJourneyDetailState extends State<CollapsibleJourneyDetail> {
  static const double _dismissDistance = 56;
  static const double _dismissVelocity = 650;

  bool _isHidden = false;
  bool _isDragging = false;
  double _dragOffset = 0;

  void _onDragStart(DragStartDetails _) {
    setState(() => _isDragging = true);
  }

  void _onDragUpdate(DragUpdateDetails details) {
    final delta = details.primaryDelta ?? 0;
    setState(() {
      _dragOffset = (_dragOffset + delta).clamp(0.0, 180.0).toDouble();
    });
  }

  void _onDragEnd(DragEndDetails details) {
    final velocity = details.primaryVelocity ?? 0;
    final shouldHide =
        _dragOffset >= _dismissDistance || velocity >= _dismissVelocity;
    if (shouldHide) {
      FocusScope.of(context).unfocus();
      AppHaptics.light();
    }
    setState(() {
      _isDragging = false;
      _isHidden = shouldHide;
      _dragOffset = 0;
    });
  }

  void _onDragCancel() {
    setState(() {
      _isDragging = false;
      _dragOffset = 0;
    });
  }

  void _restore() {
    AppHaptics.light();
    setState(() => _isHidden = false);
  }

  @override
  Widget build(BuildContext context) {
    final title = context.tr('journey.journey_info_page_title');

    // Keep the form mounted while collapsed so its draft and in-flight save
    // survive. AnimatedCrossFade also excludes the hidden child's input/focus.
    return AnimatedCrossFade(
      duration: const Duration(milliseconds: 220),
      reverseDuration: const Duration(milliseconds: 180),
      firstCurve: Curves.easeOutCubic,
      secondCurve: Curves.easeOutCubic,
      sizeCurve: Curves.easeOutCubic,
      alignment: Alignment.bottomCenter,
      clipBehavior: Clip.none,
      crossFadeState: _isHidden
          ? CrossFadeState.showFirst
          : CrossFadeState.showSecond,
      layoutBuilder: (topChild, topKey, bottomChild, bottomKey) => Stack(
        clipBehavior: Clip.none,
        alignment: Alignment.bottomCenter,
        children: [
          Positioned(
            key: bottomKey,
            left: 0,
            right: 0,
            bottom: 0,
            child: bottomChild,
          ),
          Positioned(key: topKey, child: topChild),
        ],
      ),
      firstChild: Align(
        key: const ValueKey('journey-detail-restore'),
        alignment: Alignment.bottomCenter,
        heightFactor: 1,
        child: SizedBox(
          width: 68,
          height: 32,
          child: JourneyInfoPanelSurface(
            backgroundAlpha: 0.76,
            child: Tooltip(
              message: title,
              child: InkWell(
                onTap: _restore,
                borderRadius: BorderRadius.circular(24),
                child: Center(
                  child: Icon(
                    Icons.keyboard_arrow_up_rounded,
                    size: 23,
                    color: context.appColors.deepGreen,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
      secondChild: AnimatedContainer(
        key: const ValueKey('journey-detail-card'),
        duration: _isDragging
            ? Duration.zero
            : const Duration(milliseconds: 180),
        curve: Curves.easeOutCubic,
        transform: Matrix4.translationValues(0, _dragOffset, 0),
        child: JourneyInfoPanelSurface(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              SizedBox(
                height: 19,
                child: GestureDetector(
                  key: const ValueKey('journey-detail-drag-handle'),
                  behavior: HitTestBehavior.opaque,
                  onVerticalDragStart: _onDragStart,
                  onVerticalDragUpdate: _onDragUpdate,
                  onVerticalDragEnd: _onDragEnd,
                  onVerticalDragCancel: _onDragCancel,
                  child: Center(
                    child: Container(
                      width: 34,
                      height: 4,
                      decoration: BoxDecoration(
                        color: context.appColors.mutedInkColor.withValues(
                          alpha: 0.42,
                        ),
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                  ),
                ),
              ),
              Flexible(
                fit: FlexFit.loose,
                child: SingleChildScrollView(child: widget.child),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class JourneyDetailCard extends StatelessWidget {
  const JourneyDetailCard({
    super.key,
    required this.journey,
    required this.isEditing,
    this.isSaving = false,
    required this.onExport,
    required this.onEdit,
    required this.onMore,
    required this.onCancel,
    required this.onSave,
  });

  final JourneyHeader journey;
  final bool isEditing;
  final bool isSaving;
  final VoidCallback onExport;
  final VoidCallback onEdit;
  final VoidCallback onMore;
  final VoidCallback onCancel;
  final Future<void> Function(JourneyInfo) onSave;

  @override
  Widget build(BuildContext context) {
    if (isEditing) {
      return JourneyInfoForm(
        key: ValueKey(journey.id),
        journeyDate: journey.journeyDate.toSimpleDate(),
        startTime: journey.start,
        endTime: journey.end,
        note: journey.note,
        journeyKind: journey.journeyKind,
        hasRawData: journey.hasRawData,
        isSaving: isSaving,
        onCancel: onCancel,
        onSave: onSave,
      );
    }
    final note = journey.note?.trim();
    return Padding(
      padding: const EdgeInsets.fromLTRB(14, 0, 14, 13),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const JourneyInfoCardHeader(),
          const SizedBox(height: 5),
          CompactJourneyInfoField(
            icon: Icons.calendar_today_rounded,
            label: context.tr('journey.journey_date'),
            value: journey.journeyDate.toSimpleDate().toString(),
          ),
          CompactJourneyInfoField(
            icon: Icons.sell_outlined,
            label: context.tr('journey.journey_kind'),
            value: journeyKindLabel(context, journey.journeyKind),
          ),
          CompactJourneyInfoField(
            icon: Icons.schedule_rounded,
            label: context.tr('journey.start_time'),
            value: formatJourneyInfoTime(journey.start),
          ),
          CompactJourneyInfoField(
            icon: Icons.schedule_rounded,
            label: context.tr('journey.end_time'),
            value: formatJourneyInfoTime(journey.end),
          ),
          JourneyRawDataInfoField(hasRawData: journey.hasRawData),
          if (note != null && note.isNotEmpty)
            CompactJourneyInfoField(
              icon: Icons.notes_rounded,
              label: context.tr('journey.note'),
              value: note,
              maxLines: 2,
            ),
          const SizedBox(height: 10),
          Row(
            key: const ValueKey('journey-actions'),
            children: [
              Expanded(
                child: AppButton(
                  size: AppButtonSize.compact,
                  expand: true,
                  icon: Icons.ios_share_rounded,
                  label: context.tr('common.export'),
                  variant: AppButtonVariant.secondary,
                  onPressed: onExport,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: AppButton(
                  size: AppButtonSize.compact,
                  expand: true,
                  icon: Icons.edit_outlined,
                  label: context.tr('common.edit'),
                  onPressed: onEdit,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: AppButton(
                  size: AppButtonSize.compact,
                  expand: true,
                  icon: Icons.more_horiz_rounded,
                  label: context.tr('common.more'),
                  variant: AppButtonVariant.secondary,
                  onPressed: onMore,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
