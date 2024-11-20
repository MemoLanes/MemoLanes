import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/journey_edit.dart';
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
          setState(() {
            this.journeyInfo = journeyInfo;
            this.rawBitmapData = rawBitmapData;
          });
          break;
        case ImportType.gpxOrKml:
          var (journeyInfo, rawVectorData) =
              await import_api.loadGpxOrKml(filePath: path);
          setState(() {
            this.journeyInfo = journeyInfo;
            this.rawVectorData = rawVectorData;
          });
          break;
      }
    } catch (e) {
      Fluttertoast.showToast(msg: "Data parsing failed");
      Navigator.pop(context);
    }
  }

  _saveData(import_api.JourneyInfo journeyInfo, bool runPreprocessor) async {
    if (rawVectorData == null && rawBitmapData == null) {
      Fluttertoast.showToast(msg: "JourneyData is empty");
      return;
    }

    if (rawVectorData != null) {
      await import_api.importVector(
          journeyInfo: journeyInfo,
          vectorData: rawVectorData!,
          runPreprocessor: runPreprocessor);
    } else if (rawBitmapData != null) {
      await import_api.importBitmap(
          journeyInfo: journeyInfo, bitmapData: rawBitmapData!);
    }
    Fluttertoast.showToast(msg: "Import successful");
  }

  @override
  Widget build(BuildContext context) {
    var journeyInfo = this.journeyInfo;
    return Scaffold(
      appBar: AppBar(
        title: const Text("Import Data"),
      ),
      body: Center(
        child: journeyInfo == null
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
            : JourneyInfoEditor(
                startTime: journeyInfo.startTime,
                endTime: journeyInfo.endTime,
                journeyDate: journeyInfo.journeyDate,
                note: journeyInfo.note,
                saveData: _saveData,
                importType: widget.importType,
              ),
      ),
    );
  }
}
