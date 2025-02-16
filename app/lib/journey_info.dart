import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/component/base_map_webview.dart';
import 'package:memolanes/journey_edit.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

class JourneyInfoPage extends StatefulWidget {
  const JourneyInfoPage({super.key, required this.journeyHeader});

  final JourneyHeader journeyHeader;

  @override
  State<JourneyInfoPage> createState() => _JourneyInfoPage();
}

class _JourneyInfoPage extends State<JourneyInfoPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  api.MapRendererProxy? _mapRendererProxy;

  @override
  void initState() {
    super.initState();
    api
        .getMapRendererProxyForJourney(journeyId: widget.journeyHeader.id)
        .then((mapRendererProxyAndCameraOption) {
      setState(() {
        _mapRendererProxy = mapRendererProxyAndCameraOption.$1;
      });
    });
  }

  _export(JourneyHeader journeyHeader, api.ExportType exportType) async {
    var tmpDir = await getTemporaryDirectory();
    var filepath =
        "${tmpDir.path}/${journeyHeader.revision}.${exportType.name}";
    await api.exportJourney(
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

  _saveData(JourneyInfo journeyInfo) async {
    await api.updateJourneyMetadata(
        id: widget.journeyHeader.id, journeyinfo: journeyInfo);
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
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
                "Start Time: ${widget.journeyHeader.start != null ? fmt.format(widget.journeyHeader.start!.toLocal()) : ""}"),
            Text(
                "End Time: ${widget.journeyHeader.end != null ? fmt.format(widget.journeyHeader.end!.toLocal()) : ""}"),
            Text(
                "Created At: ${fmt.format(widget.journeyHeader.createdAt.toLocal())}"),
            Text("Revision: ${widget.journeyHeader.revision}"),
            Text("Note: ${widget.journeyHeader.note}"),
            Row(mainAxisAlignment: MainAxisAlignment.spaceEvenly, children: [
              ElevatedButton(
                onPressed: () async {
                  var tmpDir = await getTemporaryDirectory();
                  var filepath =
                      "${tmpDir.path}/${widget.journeyHeader.revision}.mldx";
                  await api.generateSingleArchive(
                      journeyId: widget.journeyHeader.id,
                      targetFilepath: filepath);
                  await Share.shareXFiles([XFile(filepath)]);
                  try {
                    var file = File(filepath);
                    await file.delete();
                  } catch (e) {
                    print(e);
                  }
                },
                child: const Text("Export MLDX"),
              ),
              ElevatedButton(
                onPressed: widget.journeyHeader.journeyType ==
                        JourneyType.vector
                    ? () => _export(widget.journeyHeader, api.ExportType.kml)
                    : null,
                child: const Text("Export KML"),
              ),
              ElevatedButton(
                onPressed: widget.journeyHeader.journeyType ==
                        JourneyType.vector
                    ? () => _export(widget.journeyHeader, api.ExportType.gpx)
                    : null,
                child: const Text("Export GPX"),
              ),
            ]),
            Row(mainAxisAlignment: MainAxisAlignment.spaceEvenly, children: [
              ElevatedButton(
                onPressed: () async {
                  final result = await Navigator.push(context,
                      MaterialPageRoute(builder: (context) {
                    return Scaffold(
                      appBar: AppBar(
                        title: const Text("Edit journey Info"),
                      ),
                      body: Center(
                        child: JourneyInfoEditor(
                          startTime: widget.journeyHeader.start,
                          endTime: widget.journeyHeader.end,
                          journeyDate: widget.journeyHeader.journeyDate,
                          note: widget.journeyHeader.note,
                          saveData: _saveData,
                        ),
                      ),
                    );
                  }));
                  if (result == true) {
                    // TODO: We should just refresh the page instead of closing it.
                    if (!context.mounted) return;
                    Navigator.pop(context, true);
                  }
                },
                child: const Text("Edit"),
              ),
              ElevatedButton(
                onPressed: () async {
                  if (await showCommonDialog(
                      context, context.tr("journey.delete_journey_message"),
                      hasCancel: true,
                      title: context.tr("journey.delete_journey_title"),
                      confirmText: context.tr("journey.delete"),
                      confirmGroundColor: Colors.red,
                      confirmTextColor: Colors.white)) {
                    await api.deleteJourney(journeyId: widget.journeyHeader.id);
                    if (!context.mounted) return;
                    Navigator.pop(context, true);
                  }
                },
                child: const Text("Delete"),
              ),
            ]),
            Expanded(
              child: mapRendererProxy == null
                  ? (const CircularProgressIndicator())
                  : (BaseMapWebview(
                      key: const ValueKey("mapWidget"),
                      mapRendererProxy: mapRendererProxy,
                    )),
            )
          ],
        ),
      ),
    );
  }
}
