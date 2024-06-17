import 'dart:async';

import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:intl/intl.dart';
import 'package:project_dv/src/rust/api/api.dart';

class ImportDataPage extends StatefulWidget {
  const ImportDataPage({super.key, required this.path, this.importType});

  final String path;
  final ImportType? importType;

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

class _ImportDataPage extends State<ImportDataPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  DateTime? _startTime;
  DateTime? _endTime;
  DateTime? _journeyDate;
  final TextEditingController _noteController = TextEditingController();
  bool _runPreprocessor = false;
  bool loadCompleted = false;
  JourneyInfo? journeyInfo;

  @override
  void initState() {
    super.initState();
    _readData(widget.path);
  }

  ImportType? getType(String fileExtension) {
    switch (fileExtension) {
      case "zip":
        return ImportType.fow;
      case "gpx":
        return ImportType.gpx;
      case "kml":
        return ImportType.kml;
      default:
        Fluttertoast.showToast(msg: "Invalid file type selected");
        Navigator.pop(context);
    }
    return null;
  }

  _readData(path) async {
    ImportType? type = widget.importType;
    type ??= getType(widget.path.split('.').last.toLowerCase());
    journeyInfo = await readImportData(
      filePath: path,
      importType: type!,
      runPreprocessor: _runPreprocessor,
    );
    if (journeyInfo != null) {
      setState(() {
        _startTime = journeyInfo?.startTime;
        _endTime = journeyInfo?.endTime;
        _journeyDate = journeyInfo?.journeyDate;
      });
    }
    setState(() {
      loadCompleted = true;
    });
  }

  _saveData() async {
    if (journeyInfo != null) {
      await saveImportJourney(
          journeyInfo: JourneyInfo(
              journeyDate: _journeyDate,
              startTime: _startTime,
              endTime: _endTime,
              note: _noteController.text,
              journeyData: journeyInfo!.journeyData));
      Fluttertoast.showToast(msg: "Import successful");
      Navigator.pop(context);
    } else {
      Fluttertoast.showToast(msg: "JourneyData is empty");
    }
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

  _infoEdit(){
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
        Switch(
          value: _runPreprocessor,
          onChanged: (value) {
            setState(() {
              _runPreprocessor = value;
            });
          },
        ),
        Text(
          _runPreprocessor ? 'Preprocessor is ON' : 'Preprocessor is OFF',
          style: TextStyle(fontSize: 18.0),
        ),
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
          child: loadCompleted == false?const CircularProgressIndicator():_infoEdit(),
        ),
      );
    }
}
