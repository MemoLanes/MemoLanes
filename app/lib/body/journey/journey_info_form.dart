import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/body/map/overlay/journey_time_picker_dialog.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_date_picker_dialog.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/src/rust/api/import.dart' show JourneyInfo;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_colors.dart';

/// Shared editable journey metadata. The caller owns scrolling, save and exit.
class JourneyInfoForm extends StatefulWidget {
  const JourneyInfoForm({
    super.key,
    required this.journeyDate,
    this.startTime,
    this.endTime,
    this.note,
    required this.journeyKind,
    this.hasRawData,
    this.extraFields,
    this.isSaving,
    this.onCancel,
    required this.onSave,
  });

  final SimpleDate journeyDate;
  final DateTime? startTime;
  final DateTime? endTime;
  final String? note;
  final JourneyKind journeyKind;
  final bool? hasRawData;
  final Widget? extraFields;

  /// A flow controller can own saving; otherwise the form tracks onSave.
  final bool? isSaving;
  final VoidCallback? onCancel;
  final FutureOr<void> Function(JourneyInfo) onSave;

  @override
  State<JourneyInfoForm> createState() => _JourneyInfoFormState();
}

class _JourneyInfoFormState extends State<JourneyInfoForm> {
  static final _firstDate = DateTime(1970);
  late SimpleDate _journeyDate;
  DateTime? _startTime;
  DateTime? _endTime;
  late JourneyKind _journeyKind;
  final _noteController = TextEditingController();
  bool _saving = false;

  bool get _isSaving => widget.isSaving ?? _saving;

  @override
  void initState() {
    super.initState();
    _journeyDate = widget.journeyDate;
    _startTime = widget.startTime;
    _endTime = widget.endTime;
    _journeyKind = widget.journeyKind;
    _noteController.text = widget.note ?? '';
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
    final time = await showCompactJourneyTimeDialog(
      context,
      initialTime: TimeOfDay.fromDateTime(seed),
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
    final selected = await showAppDialog<JourneyKind>(
      context,
      surfaceBuilder: (child) =>
          AppDialogSurface(style: AppDialogSurfaceStyle.glass, child: child),
      barrierColor: context.appColors.shadowColor.withValues(
        alpha: context.appColors.pickerBarrierAlpha,
      ),
      builder: (dialogContext) => AppDialogContent(
        title: context.tr('journey.journey_kind'),
        contentPadding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            AppOptionTile(
              backgroundAlpha: 0.5,
              iconWidget: JourneyKindIcon(
                kind: JourneyKind.defaultKind,
                color: dialogContext.appColors.deepGreen,
                size: 20,
              ),
              title: journeyKindLabel(context, JourneyKind.defaultKind),
              selected: _journeyKind == JourneyKind.defaultKind,
              trailing: AppOptionTileTrailing.selection,
              onTap: () =>
                  Navigator.of(dialogContext).pop(JourneyKind.defaultKind),
            ),
            const SizedBox(height: 8),
            AppOptionTile(
              backgroundAlpha: 0.5,
              iconWidget: JourneyKindIcon(
                kind: JourneyKind.flight,
                color: dialogContext.appColors.deepGreen,
                size: 20,
              ),
              title: journeyKindLabel(context, JourneyKind.flight),
              selected: _journeyKind == JourneyKind.flight,
              trailing: AppOptionTileTrailing.selection,
              onTap: () => Navigator.of(dialogContext).pop(JourneyKind.flight),
            ),
          ],
        ),
      ),
    );
    if (selected == null || !mounted) return;
    setState(() => _journeyKind = selected);
  }

  Future<void> _save() async {
    if (_saving || _isSaving) return;
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
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.fromLTRB(14, 0, 14, 13),
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        const JourneyInfoCardHeader(),
        const SizedBox(height: 5),
        CompactJourneyInfoField(
          icon: Icons.calendar_today_rounded,
          label: context.tr('journey.journey_date'),
          value: _journeyDate.toString(),
          onTap: _isSaving ? null : _selectJourneyDate,
        ),
        CompactJourneyInfoField(
          icon: Icons.sell_outlined,
          label: context.tr('journey.journey_kind'),
          value: journeyKindLabel(context, _journeyKind),
          onTap: _isSaving ? null : _selectJourneyKind,
        ),
        CompactJourneyInfoField(
          icon: Icons.schedule_rounded,
          label: context.tr('journey.start_time'),
          value: formatJourneyInfoTime(_startTime),
          onTap: _isSaving
              ? null
              : () async {
                  final selected = await _selectDateAndTime(_startTime);
                  if (selected != null && mounted) {
                    setState(() => _startTime = selected);
                  }
                },
        ),
        CompactJourneyInfoField(
          icon: Icons.schedule_rounded,
          label: context.tr('journey.end_time'),
          value: formatJourneyInfoTime(_endTime),
          onTap: _isSaving
              ? null
              : () async {
                  final selected = await _selectDateAndTime(_endTime);
                  if (selected != null && mounted) {
                    setState(() => _endTime = selected);
                  }
                },
        ),
        if (widget.hasRawData case final hasRawData?)
          JourneyRawDataInfoField(hasRawData: hasRawData),
        ?widget.extraFields,
        CompactJourneyInfoField(
          icon: Icons.notes_rounded,
          label: context.tr('journey.note'),
          trailing: TextField(
            controller: _noteController,
            readOnly: _isSaving,
            minLines: 1,
            maxLines: 2,
            textAlign: TextAlign.right,
            style: AppTypography.supporting.copyWith(
              color: context.appColors.deepGreen,
              fontWeight: FontWeight.w600,
            ),
            decoration: InputDecoration(
              isDense: true,
              border: InputBorder.none,
              hintText: context.tr('common.please_enter'),
              hintStyle: AppTypography.supporting.copyWith(
                color: context.appColors.mutedInkColor,
              ),
            ),
          ),
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            if (widget.onCancel != null) ...[
              Expanded(
                child: AppButton(
                  size: AppButtonSize.compact,
                  expand: true,
                  label: context.tr('common.cancel'),
                  variant: AppButtonVariant.secondary,
                  onPressed: _isSaving ? null : widget.onCancel,
                ),
              ),
              const SizedBox(width: 8),
            ],
            Expanded(
              child: AppButton(
                size: AppButtonSize.compact,
                expand: true,
                icon: Icons.check_rounded,
                label: context.tr('common.save'),
                onPressed: _isSaving ? null : _save,
                loading: _isSaving,
              ),
            ),
          ],
        ),
      ],
    ),
  );
}
