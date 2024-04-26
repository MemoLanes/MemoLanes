import 'dart:async';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:intl/intl.dart';
import 'package:project_dv/src/rust/api/api.dart';

class ImportDataPage extends StatefulWidget {
  const ImportDataPage({super.key});

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

class _ImportDataPage extends State<ImportDataPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  DateTime? _startTime;
  DateTime? _endTime;
  final TextEditingController _noteController = TextEditingController();
  ImportType _importType = ImportType.fow;
  bool _runPreprocessor = false;
  JourneyInfo? journeyInfo;

  String getExtension(ImportType type) {
    if (type == ImportType.fow) {
      return "zip";
    }
    if (type == ImportType.gpx) {
      return "gpx";
    }
    if (type == ImportType.kml) {
      return "kml";
    }
    return "";
  }

  _readData() async {

      var extension = getExtension(_importType);
      FilePickerResult? result;
      try {
        result = await FilePicker.platform
            .pickFiles(type: FileType.custom, allowedExtensions: [extension]);
      } on PlatformException {
        result = await FilePicker.platform.pickFiles(type: FileType.any);
      } catch (e) {
        rethrow;
      }
      if (result != null) {
        final path = result.files.single.path;
        final fileExtension = path?.split('.').last.toLowerCase();

        if (fileExtension != extension ){
          Fluttertoast.showToast(msg: "file type error");
          return;
        }
        if (path != null ) {
          journeyInfo = await readImportData(
              filePath: path,
              importType: _importType,
              runPreprocessor: _runPreprocessor,
          );
          if (journeyInfo !=null){
            setState(() {
              _startTime = journeyInfo?.startTime;
              _endTime = journeyInfo?.endTime;
            });
          }

        }
      }

  }

  _saveData() async {
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text("Import Data"),
      ),
      body: Center(
        child: Column(
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
              controller: _noteController,
              decoration: const InputDecoration(
                label: Text("note"),
              ),
            ),
            RadioListTile<ImportType>(
              value: ImportType.kml,
              title: const Text('kml'),
              groupValue: _importType,
              onChanged: (value) {
                setState(() {
                  _importType = value!;
                });
              },
            ),
            RadioListTile<ImportType>(
              value: ImportType.gpx,
              title: const Text('gpx'),
              groupValue: _importType,
              onChanged: (value) {
                setState(() {
                  _importType = value!;
                });
              },
            ),
            RadioListTile<ImportType>(
              value: ImportType.fow,
              title: const Text('FoW'),
              groupValue: _importType,
              onChanged: (value) {
                setState(() {
                  _importType = value!;
                });
              },
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
              onPressed: _readData,
              child: const Text("read data"),
            ),
            ElevatedButton(
              onPressed: _saveData,
              child: const Text("save data"),
            ),
          ],
        ),
      ),
    );
  }
}
