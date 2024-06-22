import 'dart:async';

import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:intl/intl.dart';
import 'package:project_dv/src/rust/api/import.dart';

class ImportDataPage extends StatefulWidget {
  const ImportDataPage({super.key, required this.path, this.importType});

  final String path;
  final ImportType? importType;

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

enum ImportType { fow, kml, gpx }

class _ImportDataPage extends State<ImportDataPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  DateTime? _startTime;
  DateTime? _endTime;
  DateTime? _journeyDate;
  final TextEditingController _noteController = TextEditingController();
  bool _runPreprocessor = false;
  bool loadCompleted = false;
  late ImportType importType;
  JourneyInfo? journeyInfo;
  RawBitmapData? rawBitmapData;
  RawVectorData? rawVectorData;

  @override
  void initState() {
    super.initState();
    if (widget.importType != null) {
      importType = widget.importType!;
    } else {
      try {
        importType = getType(widget.path.split('.').last.toLowerCase());
      } catch (e) {
        Fluttertoast.showToast(msg: "Invalid file type selected");
        Navigator.pop(context);
        return;
      }
    }
    _readData(widget.path);
  }

  ImportType getType(String fileExtension) {
    ImportType importType;
    switch (fileExtension) {
      case "gpx":
        importType = ImportType.gpx;
      case "kml":
        importType = ImportType.kml;
      default:
        throw "Invalid file type selected";
    }
    return importType;
  }

  _readData(path) async {
    try {
      switch (importType) {
        case ImportType.fow:
          var (JourneyInfo journeyInfo, RawBitmapData rawBitmapData) =
              await loadFowSyncData(filePath: path);
          this.journeyInfo = journeyInfo;
          this.rawBitmapData = rawBitmapData;
          break;
        case ImportType.kml:
          var (JourneyInfo journeyInfo, RawVectorData rawVectorData) =
              await loadKml(filePath: path);
          this.journeyInfo = journeyInfo;
          this.rawVectorData = rawVectorData;
          break;
        case ImportType.gpx:
          var (JourneyInfo journeyInfo, RawVectorData rawVectorData) =
              await loadGpx(filePath: path);
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
      await importVector(
          journeyInfo: journeyInfo!,
          vectorData: rawVectorData!,
          runPreprocessor: _runPreprocessor);
    } else if (rawBitmapData != null) {
      await importBitmap(journeyInfo: journeyInfo!, bitmapData: rawBitmapData!);
    }
    Fluttertoast.showToast(msg: "Import successful");
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
            text: _startTime != null ? fmt.format(_startTime!) : '',
          ),
          onTap: () async {
            DateTime? time = await selectDateAndTime(context);
            setState(() {
              _startTime = time;
            });
          },
          decoration: const InputDecoration(
            label: Text("startTime"),
          ),
        ),
        TextField(
          readOnly: true,
          controller: TextEditingController(
            text: _endTime != null ? fmt.format(_endTime!) : '',
          ),
          onTap: () async {
            DateTime? time = await selectDateAndTime(context);
            setState(() {
              _endTime = time;
            });
          },
          decoration: const InputDecoration(
            label: Text("endTime"),
          ),
        ),
        TextField(
          readOnly: true,
          controller: TextEditingController(
            text: _journeyDate != null ? fmt.format(_journeyDate!) : '',
          ),
          onTap: () async {
            DateTime? time = await selectDateAndTime(context);
            setState(() {
              _journeyDate = time;
            });
          },
          decoration: const InputDecoration(
            label: Text("journeyDate"),
          ),
        ),
        TextField(
          controller: _noteController,
          decoration: const InputDecoration(
            label: Text("note"),
          ),
        ),
        importType == ImportType.fow
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
                  style: TextStyle(fontSize: 18.0),
                )
              ]),
        ElevatedButton(
          onPressed: _saveData,
          child: const Text("save data"),
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
