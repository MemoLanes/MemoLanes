import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/body/journey/journey_info_fields.dart';
import 'package:memolanes/body/settings/import_data_page.dart' show ImportType;
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_date_picker_dialog.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyInfoEditPage extends StatefulWidget {
  const JourneyInfoEditPage({
    super.key,
    required this.startTime,
    required this.endTime,
    required this.journeyDate,
    required this.note,
    required this.saveData,
    this.previewData,
    this.journeyKind,
    this.importType,
    this.preprocessor,
  });

  final DateTime? startTime;
  final DateTime? endTime;
  final SimpleDate journeyDate;
  final String? note;
  final JourneyKind? journeyKind;
  final FutureOr<void> Function(
    import_api.JourneyInfo journeyInfo,
    import_api.ImportPreprocessor preprocessor,
  )
  saveData;
  final FutureOr<void> Function(import_api.ImportPreprocessor preprocessor)?
  previewData;
  final ImportType? importType;
  final import_api.ImportPreprocessor? preprocessor;

  @override
  State<JourneyInfoEditPage> createState() => _JourneyInfoEditPageState();
}

class _JourneyInfoEditPageState extends State<JourneyInfoEditPage> {
  final DateFormat dateTimeFormat = DateFormat('yyyy-MM-dd HH:mm:ss');
  final SimpleDate _firstSelectableDate = SimpleDate(1970);
  DateTime? _startTime;
  DateTime? _endTime;
  SimpleDate? _journeyDate;
  JourneyKind _journeyKind = JourneyKind.defaultKind;
  bool _saving = false;
  final TextEditingController _noteController = TextEditingController();
  late import_api.ImportPreprocessor _preprocessor;

  Future<DateTime?> selectDateAndTime(
    BuildContext context,
    DateTime? datetime,
  ) async {
    final now = DateTime.now();
    final localDateTime = datetime?.toLocal() ?? now;
    DateTime? selectedDateTime = await showAppDatePickerDialog(
      context,
      initialDate: localDateTime,
      firstDate: _firstSelectableDate.toLocalDateTime(),
      lastDate: now,
      highlightInitialDate: true,
    );

    if (selectedDateTime == null) return null;
    if (!context.mounted) return null;

    TimeOfDay? selectedTime = await showTimePicker(
      context: context,
      initialTime: TimeOfDay.fromDateTime(localDateTime),
    );

    if (selectedTime == null) return null;

    return DateTime(
      selectedDateTime.year,
      selectedDateTime.month,
      selectedDateTime.day,
      selectedTime.hour,
      selectedTime.minute,
    );
  }

  @override
  void initState() {
    super.initState();
    _startTime = widget.startTime?.toLocal();
    _endTime = widget.endTime?.toLocal();
    _journeyDate = widget.journeyDate;
    _journeyKind = widget.journeyKind ?? _journeyKind;
    _noteController.text = widget.note ?? "";
    _preprocessor =
        widget.preprocessor ?? import_api.ImportPreprocessor.generic;
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  Future<void> _saveData(BuildContext context) async {
    if (_saving) return;
    if (_journeyDate == null) {
      Fluttertoast.showToast(msg: context.tr("journey.journey_date_is_empty"));
      return;
    }
    setState(() => _saving = true);
    try {
      final journeyInfo = import_api.JourneyInfo(
        journeyDate: _journeyDate!.toFrbNaiveDate(),
        startTime: _startTime,
        endTime: _endTime,
        note: _noteController.text,
        journeyKind: _journeyKind,
      );
      await widget.saveData(journeyInfo, _preprocessor);
      if (!context.mounted) return;
      popCurrentRoute(context, true);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: BoxConstraints(maxHeight: 440, minHeight: 420),
      child: MlSingleChildScrollView(
        padding: const EdgeInsets.symmetric(vertical: 16.0),
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          LabelTile(
            label: context.tr("journey.start_time"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
              content: _startTime != null
                  ? dateTimeFormat.format(_startTime!)
                  : "",
            ),
            onTap: () async {
              DateTime? time = await selectDateAndTime(context, _startTime);
              if (time != null) {
                setState(() {
                  _startTime = time;
                });
              }
            },
          ),
          LabelTile(
            label: context.tr("journey.end_time"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
              content: _endTime != null ? dateTimeFormat.format(_endTime!) : "",
            ),
            onTap: () async {
              DateTime? time = await selectDateAndTime(context, _endTime);
              if (time != null) {
                setState(() {
                  _endTime = time;
                });
              }
            },
          ),
          LabelTile(
            label: context.tr("journey.journey_date"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
              content: _journeyDate != null ? _journeyDate!.toString() : '',
            ),
            onTap: () async {
              DateTime? time = await showAppDatePickerDialog(
                context,
                initialDate: _journeyDate?.toLocalDateTime() ?? DateTime.now(),
                firstDate: _firstSelectableDate.toLocalDateTime(),
                lastDate: DateTime.now(),
                highlightInitialDate: true,
              );
              if (time != null) {
                setState(() {
                  _journeyDate = time.toSimpleDate();
                });
              }
            },
          ),
          if (widget.importType != null)
            widget.importType == ImportType.fow
                ? SizedBox.shrink()
                : ImportPreprocessorTile(
                    value: _preprocessor,
                    onSelected: _selectPreprocessor,
                    onInfoTap: () => showCommonDialog(
                      context,
                      context.tr("import.preprocessor.description_md"),
                      markdown: true,
                    ),
                  ),
          JourneyKindTile(
            value: _journeyKind,
            onSelected: (value) => setState(() => _journeyKind = value),
          ),
          JourneyNoteTile(controller: _noteController),
          SizedBox(
            width: 280,
            child: AppButton(
              label: context.tr("common.save"),
              onPressed: () async => _saveData(context),
              loading: _saving,
              expand: true,
            ),
          ),
        ],
      ),
    );
  }

  void _selectPreprocessor(import_api.ImportPreprocessor processor) {
    if (_preprocessor == processor) return;
    setState(() {
      _preprocessor = processor;
    });
    widget.previewData?.call(_preprocessor);
  }
}
