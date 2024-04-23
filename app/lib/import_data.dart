import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
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
                text: _endTime != null
                    ? fmt.format(_endTime!)
                    : '',
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
            ElevatedButton(
              onPressed: () async {
                var result = await FilePicker.platform.pickFiles(
                    type: FileType.custom, allowedExtensions: ['zip']);
                if (result != null) {
                  var path = result.files.single.path;
                  if (path != null) {
                    await importFowData(zipFilePath: path);
                  }
                }
              },
              child: const Text("FoW data"),
            ),
          ],
        ),
      ),
    );
  }
}
