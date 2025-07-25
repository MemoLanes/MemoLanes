import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/component/tiles/label_tile.dart';
import 'package:memolanes/component/tiles/label_tile_content.dart';
import 'package:memolanes/import_data.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils.dart';

class JourneyInfoEditor extends StatefulWidget {
  const JourneyInfoEditor({
    super.key,
    required this.startTime,
    required this.endTime,
    required this.journeyDate,
    required this.note,
    required this.saveData,
    this.previewData,
    this.journeyKind,
    this.importType,
  });

  final DateTime? startTime;
  final DateTime? endTime;
  final NaiveDate journeyDate;
  final String? note;
  final JourneyKind? journeyKind;
  final Function saveData;
  final Function? previewData;
  final ImportType? importType;

  @override
  State<JourneyInfoEditor> createState() => _JourneyInfoEditor();
}

class _JourneyInfoEditor extends State<JourneyInfoEditor> {
  final DateFormat dateTimeFormat = DateFormat('yyyy-MM-dd HH:mm:ss');
  final DateFormat dateFormat = DateFormat("yyyy-MM-dd");
  final DateTime firstDate = DateTime(1990);
  DateTime? _startTime;
  DateTime? _endTime;
  DateTime? _journeyDate;
  String? _note;
  JourneyKind _journeyKind = JourneyKind.defaultKind;
  import_api.JourneyInfo? journeyInfo;
  final TextEditingController _noteController = TextEditingController();
  bool _runPreprocessor = false;

  Future<DateTime?> selectDateAndTime(
      BuildContext context, DateTime? datetime) async {
    final now = DateTime.now();
    datetime ??= now;
    DateTime? selectedDateTime = await showDatePicker(
      context: context,
      initialDate: datetime,
      firstDate: firstDate,
      lastDate: now,
    );

    TimeOfDay initialTime =
        TimeOfDay(hour: datetime.hour, minute: datetime.minute);

    if (selectedDateTime != null) {
      if (!context.mounted) return null;
      TimeOfDay? selectedTime = await showTimePicker(
        context: context,
        initialTime: initialTime,
      );

      if (selectedTime != null) {
        selectedDateTime = DateTime(
          selectedDateTime.year,
          selectedDateTime.month,
          selectedDateTime.day,
          selectedTime.hour,
          selectedTime.minute,
        );
        return selectedDateTime;
      }
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    setState(() {
      _startTime = widget.startTime;
      _endTime = widget.endTime;
      _journeyDate =
          dateFormat.parse(naiveDateToString(date: widget.journeyDate));
      _note = widget.note;
      _journeyKind = widget.journeyKind ?? _journeyKind;
      _noteController.text = _note ?? "";
      _noteController.addListener(() {
        setState(() {
          _note = _noteController.text;
        });
      });
    });
    if (widget.previewData != null) {
      widget.previewData!(_runPreprocessor);
    }
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  void _saveData(BuildContext context) async {
    if (_journeyDate == null) {
      Fluttertoast.showToast(msg: context.tr("journey.journey_date_is_empty"));
      return;
    }
    _note ??= "";
    import_api.JourneyInfo journeyInfo = import_api.JourneyInfo(
        journeyDate: naiveDateOfString(str: dateFormat.format(_journeyDate!)),
        startTime: _startTime,
        endTime: _endTime,
        note: _note,
        journeyKind: _journeyKind);
    if (widget.importType != null) {
      await widget.saveData(journeyInfo, _runPreprocessor);
    } else {
      await widget.saveData(journeyInfo);
    }
    if (context.mounted) {
      Navigator.pop(context, true);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        LabelTile(
          label: context.tr("journey.start_time"),
          position: LabelTilePosition.single,
          trailing: LabelTileContent(
              content: _startTime != null
                  ? dateTimeFormat.format(_startTime!.toLocal())
                  : ""),
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
              content: _endTime != null
                  ? dateTimeFormat.format(_endTime!.toLocal())
                  : ""),
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
              content:
                  _journeyDate != null ? dateFormat.format(_journeyDate!) : ''),
          onTap: () async {
            DateTime? time = await showDatePicker(
              context: context,
              initialDate: _journeyDate,
              firstDate: firstDate,
              lastDate: DateTime.now(),
            );
            if (time != null) {
              setState(() {
                _journeyDate = time;
              });
            }
          },
        ),
        if (widget.importType != null)
          widget.importType == ImportType.fow
              ? SizedBox.shrink()
              : LabelTile(
                  label: context.tr("journey.preprocessor"),
                  position: LabelTilePosition.single,
                  trailing: Switch(
                    value: _runPreprocessor,
                    onChanged: (value) {
                      setState(() {
                        _runPreprocessor = value;
                      });
                      if (widget.previewData != null) {
                        widget.previewData!(value);
                      }
                    },
                  ),
                ),
        LabelTile(
          label: context.tr("journey.journey_kind"),
          position: LabelTilePosition.single,
          trailing: LabelTileContent(
              content: _journeyKind == JourneyKind.defaultKind
                  ? context.tr("journey_kind.default")
                  : context.tr("journey_kind.flight"),
              showArrow: true),
          onTap: () => showJourneyKindCard(
            context,
            onLabelTaped: (journeyKind) async {
              setState(() {
                _journeyKind = journeyKind;
              });
            },
          ),
        ),
        LabelTile(
          label: context.tr("journey.note"),
          position: LabelTilePosition.single,
          trailing: SizedBox(
            width: 200.0,
            height: 50.0,
            child: TextField(
              controller: _noteController,
              decoration: InputDecoration(
                border: InputBorder.none,
                counterText: '',
                hintText: context.tr("common.please_enter"),
                hintStyle: TextStyle(
                  fontSize: 14.0,
                ),
              ),
              textAlign: TextAlign.right,
            ),
          ),
        ),
        ElevatedButton(
          onPressed: () => _saveData(context),
          style: ElevatedButton.styleFrom(
            backgroundColor: const Color(0xFFB6E13D),
            foregroundColor: Colors.black,
            fixedSize: Size(280, 42),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(25.0),
            ),
          ),
          child: Text(context.tr("common.save")),
        ),
      ],
    );
  }
}
