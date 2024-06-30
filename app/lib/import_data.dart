import 'dart:async';

import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;

class ImportDataPage extends StatefulWidget {
  const ImportDataPage(
      {super.key, required this.path, required this.importType});

  final String path;
  final ImportType importType;

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

enum ImportType { fow, gpxOrKml }

class _ImportDataPage extends State<ImportDataPage> {
  final DateFormat dateTimeFormat = DateFormat('yyyy-MM-dd HH:mm:ss');
  final DateFormat dateFormat = DateFormat("yyyy-MM-dd");
  final DateTime firstDate = DateTime(1990);
  DateTime? _startTime;
  DateTime? _endTime;
  DateTime? _journeyDate;
  final TextEditingController _noteController = TextEditingController();
  bool _runPreprocessor = false;
  bool isLoaded = false;
  import_api.JourneyInfo? journeyInfo;
  import_api.RawBitmapData? rawBitmapData;
  import_api.RawVectorData? rawVectorData;

  @override
  void initState() {
    super.initState();
    _readData(widget.path);
  }

  _readData(path) async {
    try {
      switch (widget.importType) {
        case ImportType.fow:
          var (journeyInfo, rawBitmapData) =
              await import_api.loadFowSyncData(filePath: path);
          this.journeyInfo = journeyInfo;
          this.rawBitmapData = rawBitmapData;
          break;
        case ImportType.gpxOrKml:
          var (journeyInfo, rawVectorData) =
              await import_api.loadGpxOrKml(filePath: path);
          this.journeyInfo = journeyInfo;
          this.rawVectorData = rawVectorData;
          break;
      }
    } catch (e) {
      Fluttertoast.showToast(msg: "Data parsing failed");
      Navigator.pop(context);
    }
    if (journeyInfo != null) {
      setState(() {
        _startTime = journeyInfo?.startTime;
        _endTime = journeyInfo?.endTime;
        _journeyDate = dateFormat.parse(journeyInfo!.journeyDate);
      });
    }
    setState(() {
      isLoaded = true;
    });
  }

  _saveData() async {
    if (rawVectorData == null && rawVectorData == null) {
      Fluttertoast.showToast(msg: "JourneyData is empty");
      return;
    }
    if (_journeyDate == null) {
      Fluttertoast.showToast(msg: "JourneyDate is empty");
      return;
    }
    String? note = _noteController.text;
    if (note.isEmpty) {
      note = null;
    }
    import_api.JourneyInfo saveInfo = import_api.JourneyInfo(
        journeyDate: DateFormat('yyyy-MM-dd').format(_journeyDate!),
        startTime: _startTime,
        endTime: _endTime,
        note: note);

    if (rawVectorData != null) {
      await import_api.importVector(
          journeyInfo: saveInfo,
          vectorData: rawVectorData!,
          runPreprocessor: _runPreprocessor);
    } else if (rawBitmapData != null) {
      await import_api.importBitmap(
          journeyInfo: saveInfo, bitmapData: rawBitmapData!);
    }
    Fluttertoast.showToast(msg: "Import successful");
    if (!context.mounted) return null;
    Navigator.pop(context);
  }

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

  _infoEdit() {
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        TextField(
          readOnly: true,
          controller: TextEditingController(
            text: _startTime != null ? dateTimeFormat.format(_startTime!) : '',
          ),
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
            text: _endTime != null ? dateTimeFormat.format(_endTime!) : '',
          ),
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Import Data"),
      ),
      body: Center(
        child: !isLoaded
            ? const Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    "Reading data, please wait",
                    style: TextStyle(fontSize: 22.0),
                  ),
                  CircularProgressIndicator()
                ],
              )
            : _infoEdit(),
      ),
    );
  }
}
