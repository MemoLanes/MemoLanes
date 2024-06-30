import 'dart:io';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:share_plus/share_plus.dart';

class JourneyInfoPage extends StatefulWidget {
  const JourneyInfoPage({super.key, required this.journeyHeader});

  final JourneyHeader journeyHeader;

  @override
  State<JourneyInfoPage> createState() => _JourneyInfoPage();
}

class _JourneyInfoPage extends State<JourneyInfoPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');

  _export(JourneyHeader journeyHeader, ExportType exportType) async {
    var tmpDir = await getTemporaryDirectory();
    var filepath =
        "${tmpDir.path}/${journeyHeader.revision}.${exportType.name}";
    await exportJourney(
        targetFilepath: filepath,
        journeyId: journeyHeader.id,
        exportType: exportType);
    await Share.shareXFiles([XFile(filepath)]);
    try {
      await File(filepath).delete();
    } catch (e) {
      print(e);
      // don't care about error
    }
  }

  showDialogFunction(fn) {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text("Delete"),
          content: const Text("Delete this record?"),
          actions: [
            TextButton(
              onPressed: () {
                Navigator.of(context).pop();
              },
              child: const Text('Cancel'),
            ),
            TextButton(onPressed: fn, child: const Text("Yes")),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Journey Info"),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Text("Journey ID: ${widget.journeyHeader.id}"),
            Text(
                "Journey Date: ${naiveDateToString(date: widget.journeyHeader.journeyDate)}"),
            Text(
                "Start Time: ${widget.journeyHeader.start != null ? fmt.format(widget.journeyHeader.start!) : ""}"),
            Text(
                "End Time: ${widget.journeyHeader.end != null ? fmt.format(widget.journeyHeader.end!) : ""}"),
            Text("Created At: ${fmt.format(widget.journeyHeader.createdAt)}"),
            Text("Revision: ${widget.journeyHeader.revision}"),
            Text("Note: ${widget.journeyHeader.note}"),
            Row(mainAxisAlignment: MainAxisAlignment.spaceEvenly, children: [
              ElevatedButton(
                onPressed:
                    widget.journeyHeader.journeyType == JourneyType.vector
                        ? () => _export(widget.journeyHeader, ExportType.kml)
                        : null,
                child: const Text("export KML"),
              ),
              ElevatedButton(
                onPressed:
                    widget.journeyHeader.journeyType == JourneyType.vector
                        ? () => _export(widget.journeyHeader, ExportType.gpx)
                        : null,
                child: const Text("export GPX"),
              ),
              ElevatedButton(
                onPressed: () async {
                  showDialogFunction(() async {
                    Navigator.of(context).pop();
                    await deleteJourney(id: widget.journeyHeader.id);
                    if (!context.mounted) return;
                    Navigator.pop(context, true);
                  });
                },
                child: const Text("delete"),
              ),
            ]),
          ],
        ),
      ),
    );
  }
}
