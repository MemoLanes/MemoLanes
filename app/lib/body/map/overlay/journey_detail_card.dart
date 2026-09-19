import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_date_picker_dialog.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' show JourneyInfo;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'journey_time_picker_dialog.dart';

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
    if (shouldHide) AppHaptics.light();
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

    return AnimatedSwitcher(
      duration: const Duration(milliseconds: 220),
      reverseDuration: const Duration(milliseconds: 180),
      switchInCurve: Curves.easeOutCubic,
      switchOutCurve: Curves.easeInCubic,
      layoutBuilder: (currentChild, previousChildren) => Stack(
        alignment: Alignment.bottomCenter,
        children: [...previousChildren, ?currentChild],
      ),
      transitionBuilder: (child, animation) {
        final slide = Tween<Offset>(
          begin: const Offset(0, 0.12),
          end: Offset.zero,
        ).animate(animation);
        return FadeTransition(
          opacity: animation,
          child: SlideTransition(position: slide, child: child),
        );
      },
      child: _isHidden
          ? Align(
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
                    child: Material(
                      color: Colors.transparent,
                      child: InkWell(
                        onTap: _restore,
                        borderRadius: BorderRadius.circular(24),
                        child: Center(
                          child: Icon(
                            Icons.keyboard_arrow_up_rounded,
                            size: 23,
                            color: StyleConstants.deepGreen,
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            )
          : AnimatedContainer(
              key: const ValueKey('journey-detail-card'),
              duration: _isDragging
                  ? Duration.zero
                  : const Duration(milliseconds: 180),
              curve: Curves.easeOutCubic,
              transform: Matrix4.translationValues(0, _dragOffset, 0),
              child: Stack(
                children: [
                  widget.child,
                  Positioned(
                    left: 0,
                    right: 0,
                    top: 0,
                    height: 19,
                    child: GestureDetector(
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
                            color: StyleConstants.mutedInkColor.withValues(
                              alpha: 0.42,
                            ),
                            borderRadius: BorderRadius.circular(2),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
    );
  }
}

class JourneyDetailCard extends StatefulWidget {
  const JourneyDetailCard({
    super.key,
    required this.journey,
    required this.isEditing,
    required this.onExport,
    required this.onEdit,
    required this.onMore,
    required this.onSave,
  });

  final JourneyHeader journey;
  final bool isEditing;
  final VoidCallback onExport;
  final VoidCallback onEdit;
  final VoidCallback onMore;
  final Future<void> Function(JourneyInfo journeyInfo) onSave;

  @override
  State<JourneyDetailCard> createState() => _JourneyDetailCardState();
}

class _JourneyDetailCardState extends State<JourneyDetailCard> {
  static final DateTime _firstDate = DateTime(1970);

  late SimpleDate _journeyDate;
  DateTime? _startTime;
  DateTime? _endTime;
  late JourneyKind _journeyKind;
  final TextEditingController _noteController = TextEditingController();
  bool _saving = false;

  JourneyHeader get journey => widget.journey;

  @override
  void initState() {
    super.initState();
    _resetFields();
  }

  @override
  void didUpdateWidget(covariant JourneyDetailCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if ((!oldWidget.isEditing && widget.isEditing) ||
        (oldWidget.journey.revision != widget.journey.revision &&
            !widget.isEditing)) {
      _resetFields();
    }
  }

  void _resetFields() {
    _journeyDate = journey.journeyDate.toSimpleDate();
    _startTime = journey.start;
    _endTime = journey.end;
    _journeyKind = journey.journeyKind;
    _noteController.text = journey.note ?? '';
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  DateTime _validInitialDate(DateTime date) {
    final now = DateTime.now();
    if (date.isBefore(_firstDate)) return _firstDate;
    if (date.isAfter(now)) return now;
    return date;
  }

  Future<DateTime?> _selectDateAndTime(DateTime? current) async {
    final seed = current?.toLocal() ?? DateTime.now();
    final date = await showAppDatePickerDialog(
      context,
      initialDate: _validInitialDate(seed),
      firstDate: _firstDate,
      lastDate: DateTime.now(),
      highlightInitialDate: true,
    );
    if (date == null || !mounted) return null;
    final time = await showDialog<TimeOfDay>(
      context: context,
      barrierColor: StyleConstants.shadowColor.withValues(
        alpha: StyleConstants.isDarkMode ? 0.58 : 0.2,
      ),
      builder: (dialogContext) =>
          CompactJourneyTimeDialog(initialTime: TimeOfDay.fromDateTime(seed)),
    );
    if (time == null) return null;
    return DateTime(date.year, date.month, date.day, time.hour, time.minute);
  }

  Future<void> _selectJourneyDate() async {
    final selected = await showAppDatePickerDialog(
      context,
      initialDate: _validInitialDate(_journeyDate.toLocalDateTime()),
      firstDate: _firstDate,
      lastDate: DateTime.now(),
      highlightInitialDate: true,
    );
    if (selected == null || !mounted) return;
    setState(() => _journeyDate = selected.toSimpleDate());
  }

  Future<void> _selectJourneyKind() async {
    final selected = await showDialog<JourneyKind>(
      context: context,
      barrierColor: StyleConstants.shadowColor.withValues(
        alpha: StyleConstants.isDarkMode ? 0.58 : 0.2,
      ),
      builder: (dialogContext) => Dialog(
        elevation: 0,
        backgroundColor: Colors.transparent,
        insetPadding: const EdgeInsets.symmetric(horizontal: 42),
        child: PointerInterceptor(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 340),
            child: AppDialogCard(
              title: context.tr('journey.journey_kind'),
              surfaceStyle: AppDialogSurfaceStyle.glass,
              maxHeightFactor: 0.5,
              contentPadding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  AppOptionTile(
                    backgroundAlpha: 0.5,
                    iconWidget: JourneyKindIcon(
                      kind: JourneyKind.defaultKind,
                      color: StyleConstants.deepGreen,
                      size: 20,
                    ),
                    title: context.tr('journey_kind.default'),
                    selected: _journeyKind == JourneyKind.defaultKind,
                    trailing: AppOptionTileTrailing.selection,
                    onTap: () =>
                        Navigator.of(dialogContext)
                            .pop(JourneyKind.defaultKind),
                  ),
                  const SizedBox(height: 8),
                  AppOptionTile(
                    backgroundAlpha: 0.5,
                    iconWidget: JourneyKindIcon(
                      kind: JourneyKind.flight,
                      color: StyleConstants.deepGreen,
                      size: 20,
                    ),
                    title: context.tr('journey_kind.flight'),
                    selected: _journeyKind == JourneyKind.flight,
                    trailing: AppOptionTileTrailing.selection,
                    onTap: () =>
                        Navigator.of(dialogContext).pop(JourneyKind.flight),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    if (selected == null || !mounted) return;
    setState(() => _journeyKind = selected);
  }

  Future<void> _save() async {
    if (_saving) return;
    setState(() => _saving = true);
    try {
      await widget.onSave(
        JourneyInfo(
          journeyDate: _journeyDate.toFrbNaiveDate(),
          startTime: _startTime,
          endTime: _endTime,
          note: _noteController.text,
          journeyKind: _journeyKind,
        ),
      );
    } catch (error, stackTrace) {
      log.error('Saving journey information failed: $error', stackTrace);
      if (!mounted) return;
      await showCommonDialog(
        context,
        context.tr('journey.editor.operation_failed'),
      );
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isEditing = widget.isEditing;
    final canEdit = isEditing && !_saving;
    final displayedKind = isEditing ? _journeyKind : journey.journeyKind;
    final kind = switch (displayedKind) {
      JourneyKind.defaultKind => context.tr('journey_kind.default'),
      JourneyKind.flight => context.tr('journey_kind.flight'),
    };
    final timeFormat = DateFormat('yyyy-MM-dd HH:mm');
    final start = (isEditing ? _startTime : journey.start)?.toLocal();
    final end = (isEditing ? _endTime : journey.end)?.toLocal();
    final note = journey.note?.trim();

    return JourneyInfoPanelSurface(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 18, 14, 13),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const JourneyInfoCardHeader(),
            const SizedBox(height: 5),
            CompactJourneyInfoField(
              icon: Icons.calendar_today_rounded,
              label: context.tr('journey.journey_date'),
              value: isEditing
                  ? _journeyDate.toString()
                  : journey.journeyDate.toSimpleDate().toString(),
              onTap: canEdit ? _selectJourneyDate : null,
            ),
            CompactJourneyInfoField(
              icon: Icons.sell_outlined,
              label: context.tr('journey.journey_kind'),
              value: kind,
              onTap: canEdit ? _selectJourneyKind : null,
            ),
            CompactJourneyInfoField(
              icon: Icons.schedule_rounded,
              label: context.tr('journey.start_time'),
              value: start == null ? '—' : timeFormat.format(start),
              onTap: canEdit
                  ? () async {
                      final selected = await _selectDateAndTime(_startTime);
                      if (selected != null && mounted) {
                        setState(() => _startTime = selected);
                      }
                    }
                  : null,
            ),
            CompactJourneyInfoField(
              icon: Icons.schedule_rounded,
              label: context.tr('journey.end_time'),
              value: end == null ? '—' : timeFormat.format(end),
              onTap: canEdit
                  ? () async {
                      final selected = await _selectDateAndTime(_endTime);
                      if (selected != null && mounted) {
                        setState(() => _endTime = selected);
                      }
                    }
                  : null,
            ),
            if (isEditing)
              CompactJourneyInfoField(
                icon: Icons.notes_rounded,
                label: context.tr('journey.note'),
                trailing: TextField(
                  controller: _noteController,
                  readOnly: !canEdit,
                  minLines: 1,
                  maxLines: 2,
                  textAlign: TextAlign.right,
                  style: AppTypography.supporting.copyWith(
                    color: StyleConstants.deepGreen,
                    fontWeight: FontWeight.w600,
                  ),
                  decoration: InputDecoration(
                    isDense: true,
                    border: InputBorder.none,
                    hintText: context.tr('common.please_enter'),
                    hintStyle: AppTypography.supporting.copyWith(
                      color: StyleConstants.mutedInkColor,
                    ),
                  ),
                ),
              )
            else if (note != null && note.isNotEmpty)
              CompactJourneyInfoField(
                icon: Icons.notes_rounded,
                label: context.tr('journey.note'),
                value: note,
                maxLines: 2,
              ),
            const SizedBox(height: 10),
            AnimatedSwitcher(
              duration: const Duration(milliseconds: 220),
              child: isEditing
                  ? Center(
                      key: const ValueKey('save-journey-information'),
                      child: SizedBox(
                        width: 180,
                        child: AppButton(
                          size: AppButtonSize.compact,
                          expand: true,
                          icon: Icons.check_rounded,
                          label: context.tr('common.save'),
                          variant: AppButtonVariant.primary,
                          onPressed: _saving ? null : _save,
                          loading: _saving,
                        ),
                      ),
                    )
                  : Row(
                      key: const ValueKey('journey-actions'),
                      children: [
                        Expanded(
                          child: AppButton(
                            size: AppButtonSize.compact,
                            expand: true,
                            icon: Icons.ios_share_rounded,
                            label: context.tr('common.export'),
                            variant: AppButtonVariant.secondary,
                            onPressed: widget.onExport,
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: AppButton(
                            size: AppButtonSize.compact,
                            expand: true,
                            icon: Icons.edit_outlined,
                            label: context.tr('common.edit'),
                            variant: AppButtonVariant.primary,
                            onPressed: widget.onEdit,
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
                            onPressed: widget.onMore,
                          ),
                        ),
                      ],
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
