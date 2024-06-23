import 'dart:async';

import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:intl/intl.dart';
import 'package:project_dv/src/rust/api/import.dart' as import_api;

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
  final _dateFmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  DateTime? _startTime;
  DateTime? _endTime;
  DateTime? _journeyDate;
  final TextEditingController _noteController = TextEditingController();
  bool _runPreprocessor = false;
  bool loadCompleted = false;
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
      });
    }
    setState(() {
      loadCompleted = true;
    });
  }

  _saveData() async {
    if (journeyInfo == null) {
      Fluttertoast.showToast(msg: "JourneyData is empty");
      return;
    }
    if (rawVectorData != null) {
      await import_api.importVector(
          journeyInfo: journeyInfo!,
          vectorData: rawVectorData!,
          runPreprocessor: _runPreprocessor);
    } else if (rawBitmapData != null) {
      await import_api.importBitmap(
          journeyInfo: journeyInfo!, bitmapData: rawBitmapData!);
    }
    Fluttertoast.showToast(msg: "Import successful");
    if (!context.mounted) return null;
    Navigator.pop(context);
  }

  Future<DateTime?> selectDateAndTime(BuildContext context) async {
    DateTime? selectedDateTime = await showDatePicker(
      context: context,
      initialDate: DateTime.now(),
      firstDate: DateTime(2020),
      lastDate: DateTime(2030),
    );

    if (selectedDateTime != null) {
      if (!context.mounted) return null;
      TimeOfDay? selectedTime = await showTimePicker(
        context: context,
        initialTime: TimeOfDay.now(),
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
            text: _startTime != null ? _dateFmt.format(_startTime!) : '',
          ),
          onTap: () async {
            DateTime? time = await selectDateAndTime(context);
            setState(() {
              _startTime = time;
            });
          },
          decoration: const InputDecoration(
            label: Text("Start time:"),
          ),
        ),
        TextField(
          readOnly: true,
          controller: TextEditingController(
            text: _endTime != null ? _dateFmt.format(_endTime!) : '',
          ),
          onTap: () async {
            DateTime? time = await selectDateAndTime(context);
            setState(() {
              _endTime = time;
            });
          },
          decoration: const InputDecoration(
            label: Text("End time:"),
          ),
        ),
        TextField(
          readOnly: true,
          controller: TextEditingController(
            text: _journeyDate != null ? _dateFmt.format(_journeyDate!) : '',
          ),
          onTap: () async {
            DateTime? time = await selectDateAndTime(context);
            setState(() {
              _journeyDate = time;
            });
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
        child: loadCompleted == false
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
