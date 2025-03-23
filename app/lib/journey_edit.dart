import 'dart:async';

import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/import_data.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyInfoEditor extends StatefulWidget {
  const JourneyInfoEditor(
      {super.key,
      required this.startTime,
      required this.endTime,
      required this.journeyDate,
      required this.note,
      required this.saveData,
      this.journeyKind,
      this.importType});

  final DateTime? startTime;
  final DateTime? endTime;
  final NaiveDate journeyDate;
  final String? note;
  final JourneyKind? journeyKind;
  final Function saveData;
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
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  _saveData() async {
    if (_journeyDate == null) {
      Fluttertoast.showToast(msg: "JourneyDate is empty");
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
    var context = this.context;
    if (context.mounted) {
      Navigator.pop(context, true);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        TextField(
          readOnly: true,
          controller: TextEditingController(
              text: _startTime != null
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
          decoration: const InputDecoration(
            label: Text("Start time:"),
          ),
        ),
        TextField(
          readOnly: true,
          controller: TextEditingController(
              text: _endTime != null
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
          decoration: const InputDecoration(
            label: Text("End time:"),
          ),
        ),
        TextField(
          readOnly: true,
          controller: TextEditingController(
            text: _journeyDate != null ? dateFormat.format(_journeyDate!) : '',
          ),
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
          decoration: const InputDecoration(
            label: Text("Journey date:"),
          ),
        ),
        TextField(
          controller: _noteController,
          decoration: const InputDecoration(
            label: Text("Note:"),
          ),
        ),
        Row(
          children: [
            Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Radio(
                    value: JourneyKind.defaultKind,
                    groupValue: _journeyKind,
                    onChanged: (v) {
                      setState(() {
                        this._journeyKind = v!;
                      });
                    },
                  ),
                  Text("defaultKind")
                ]),
            Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Radio(
                    value: JourneyKind.flight,
                    groupValue: _journeyKind,
                    onChanged: (v) {
                      setState(() {
                        this._journeyKind = v!;
                      });
                    },
                  ),
                  Text("flight")
                ]),
          ],
        ),
        if (widget.importType != null)
          widget.importType == ImportType.fow
              ? Container()
              : Column(children: [
                  Switch(
                    value: _runPreprocessor,
                    onChanged: (value) {
                      setState(() {
                        _runPreprocessor = value;
                      });
                    },
                  ),
                  Text(
                    _runPreprocessor
                        ? 'Preprocessor is ON'
                        : 'Preprocessor is OFF',
                    style: const TextStyle(fontSize: 18.0),
                  )
                ]),
        ElevatedButton(
          onPressed: _saveData,
          child: const Text("Save Data"),
        ),
      ],
    );
  }
}
