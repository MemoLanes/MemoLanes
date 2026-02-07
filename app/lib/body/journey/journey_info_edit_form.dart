import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

/// 旅程信息编辑表单，与 [SettingsBody] 一致的圆角、边距、行底色（复用 [LabelTile]）。
/// 仅供 [JourneyInfoPage] 内嵌编辑使用。
class JourneyInfoEditForm extends StatefulWidget {
  const JourneyInfoEditForm({
    super.key,
    required this.initialStartTime,
    required this.initialEndTime,
    required this.initialJourneyDate,
    required this.initialNote,
    required this.initialJourneyKind,
    required this.onSave,
    this.onCancel,
    this.showCancelButton = false,
  });

  final DateTime? initialStartTime;
  final DateTime? initialEndTime;
  final NaiveDate initialJourneyDate;
  final String? initialNote;
  final JourneyKind initialJourneyKind;
  final Future<void> Function(import_api.JourneyInfo journeyInfo) onSave;
  final VoidCallback? onCancel;
  final bool showCancelButton;

  @override
  State<JourneyInfoEditForm> createState() => _JourneyInfoEditFormState();
}

class _JourneyInfoEditFormState extends State<JourneyInfoEditForm> {
  static final _firstDate = DateTime(1990);
  final _dateTimeFormat = DateFormat('yyyy-MM-dd HH:mm:ss');
  final _dateFormat = DateFormat('yyyy-MM-dd');

  late DateTime? _startTime;
  late DateTime? _endTime;
  late DateTime? _journeyDate;
  late String _note;
  late JourneyKind _journeyKind;
  final TextEditingController _noteController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _startTime = widget.initialStartTime;
    _endTime = widget.initialEndTime;
    _journeyDate = _dateFormat.parse(naiveDateToString(date: widget.initialJourneyDate));
    _note = widget.initialNote ?? '';
    _noteController.text = _note;
    _journeyKind = widget.initialJourneyKind;
    _noteController.addListener(() => setState(() => _note = _noteController.text));
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  Future<DateTime?> _selectDateAndTime(BuildContext context, DateTime? datetime) async {
    final now = DateTime.now();
    datetime ??= now;
    final d = await showDatePicker(
      context: context,
      initialDate: datetime,
      firstDate: _firstDate,
      lastDate: now,
    );
    if (d == null || !context.mounted) return null;
    final t = await showTimePicker(
      context: context,
      initialTime: TimeOfDay(hour: datetime.hour, minute: datetime.minute),
    );
    if (t == null) return null;
    return DateTime(d.year, d.month, d.day, t.hour, t.minute);
  }

  Future<void> _submit(BuildContext context) async {
    if (_journeyDate == null) {
      Fluttertoast.showToast(msg: context.tr("journey.journey_date_is_empty"));
      return;
    }
    final info = import_api.JourneyInfo(
      journeyDate: naiveDateOfString(str: _dateFormat.format(_journeyDate!)),
      startTime: _startTime,
      endTime: _endTime,
      note: _note,
      journeyKind: _journeyKind,
    );
    await widget.onSave(info);
  }

  void _showJourneyKindCard(BuildContext context) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("journey_kind.default"),
            onTap: () => setState(() => _journeyKind = JourneyKind.defaultKind),
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("journey_kind.flight"),
            onTap: () => setState(() => _journeyKind = JourneyKind.flight),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final kindName = switch (_journeyKind) {
      JourneyKind.defaultKind => context.tr("journey_kind.default"),
      JourneyKind.flight => context.tr("journey_kind.flight"),
    };
    final width = MediaQuery.sizeOf(context).width;
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final noteStyle = TextStyle(
      color: isDark ? const Color(0xFFE5E5E7) : null,
      fontSize: 14,
    );
    final hintStyle = TextStyle(
      fontSize: 14,
      color: isDark ? const Color(0xFF8E8E93) : null,
    );

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LabelTile(
          label: context.tr("journey.start_time"),
          position: LabelTilePosition.top,
          trailing: LabelTileContent(
            content: _startTime != null
                ? _dateTimeFormat.format(_startTime!.toLocal())
                : "",
          ),
          onTap: () async {
            final t = await _selectDateAndTime(context, _startTime);
            if (t != null) setState(() => _startTime = t);
          },
        ),
        LabelTile(
          label: context.tr("journey.end_time"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(
            content: _endTime != null
                ? _dateTimeFormat.format(_endTime!.toLocal())
                : "",
          ),
          onTap: () async {
            final t = await _selectDateAndTime(context, _endTime);
            if (t != null) setState(() => _endTime = t);
          },
        ),
        LabelTile(
          label: context.tr("journey.journey_date"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(
            content: _journeyDate != null ? _dateFormat.format(_journeyDate!) : '',
          ),
          onTap: () async {
            final t = await showDatePicker(
              context: context,
              initialDate: _journeyDate,
              firstDate: _firstDate,
              lastDate: DateTime.now(),
            );
            if (t != null) setState(() => _journeyDate = t);
          },
        ),
        LabelTile(
          label: context.tr("journey.journey_kind"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(content: kindName, showArrow: true),
          onTap: () => _showJourneyKindCard(context),
        ),
        LabelTile(
          label: context.tr("journey.note"),
          position: LabelTilePosition.bottom,
          maxHeight: 150,
          trailing: SizedBox(
            width: width * 0.6,
            child: TextField(
              controller: _noteController,
              keyboardType: TextInputType.multiline,
              textInputAction: TextInputAction.newline,
              maxLines: 5,
              minLines: 1,
              style: noteStyle,
              decoration: InputDecoration(
                border: InputBorder.none,
                counterText: '',
                hintText: context.tr("common.please_enter"),
                hintStyle: hintStyle,
              ),
              textAlign: TextAlign.right,
            ),
          ),
        ),
        const SizedBox(height: 16),
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            if (widget.showCancelButton) ...[
              TextButton(
                onPressed: widget.onCancel,
                child: Text(context.tr("common.cancel")),
              ),
              const SizedBox(width: 24),
            ],
            ElevatedButton(
              onPressed: () => _submit(context),
              style: ElevatedButton.styleFrom(
                backgroundColor: const Color(0xFFB6E13D),
                foregroundColor: Colors.black,
                fixedSize: const Size(120, 42),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(25.0),
                ),
              ),
              child: Text(context.tr("common.save")),
            ),
          ],
        ),
      ],
    );
  }
}
