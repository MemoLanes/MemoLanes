import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/component/base_map_webview.dart';
import 'package:memolanes/journey_edit.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

enum ExportType { mldx, kml, gpx }

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

  _saveData(JourneyInfo journeyInfo) async {
    await api.updateJourneyMetadata(
        id: widget.journeyHeader.id, journeyinfo: journeyInfo);
  }

  _deleteJourneyInfo(BuildContext context) async {
    if (await showCommonDialog(
        context, context.tr("journey.delete_journey_message"),
        hasCancel: true,
        title: context.tr("journey.delete_journey_title"),
        confirmButtonText: context.tr("journey.delete"),
        confirmGroundColor: Colors.red,
        confirmTextColor: Colors.white)) {
      await api.deleteJourney(journeyId: widget.journeyHeader.id);
      if (!context.mounted) return;
      Navigator.pop(context, true);
    }
  }

  _editJourneyInfo(BuildContext context) async {
    final result =
        await Navigator.push(context, MaterialPageRoute(builder: (context) {
      return Scaffold(
        appBar: AppBar(
          title: Text(context.tr("journey.journey_info_edit_bar_title")),
        ),
        body: Center(
          child: JourneyInfoEditor(
            startTime: widget.journeyHeader.start,
            endTime: widget.journeyHeader.end,
            journeyDate: widget.journeyHeader.journeyDate,
            note: widget.journeyHeader.note,
            journeyKind: widget.journeyHeader.journeyKind,
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
  }

  Future<String> _saveFile(
      JourneyHeader journeyHeader, ExportType exportType) async {
    var tmpDir = await getTemporaryDirectory();
    var filepath =
        "${tmpDir.path}/${journeyHeader.revision}.${exportType.name}";
    switch (exportType) {
      case ExportType.mldx:
        await api.generateSingleArchive(
            journeyId: journeyHeader.id, targetFilepath: filepath);
        break;
      case ExportType.kml:
        await api.exportJourney(
            targetFilepath: filepath,
            journeyId: journeyHeader.id,
            exportType: api.ExportType.kml);
        break;
      case ExportType.gpx:
        await api.exportJourney(
            targetFilepath: filepath,
            journeyId: journeyHeader.id,
            exportType: api.ExportType.gpx);
        break;
    }
    return filepath;
  }

  _share(JourneyHeader journeyHeader, ExportType exportType) async {
    String filepath = await _saveFile(journeyHeader, exportType);
    await Share.shareXFiles([XFile(filepath)]);
    try {
      await File(filepath).delete();
    } catch (e) {
      debugPrint('Failed to delete file: $e');
    }
  }

  _showDialog(BuildContext context, JourneyHeader journeyHeader,
      ExportType exportType) {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: Text(context.tr("journey.export_journey_data_title")),
          content: Row(
            mainAxisAlignment: MainAxisAlignment.spaceAround,
            children: [
              Column(
                mainAxisAlignment: MainAxisAlignment.center,
                mainAxisSize: MainAxisSize.min,
                children: [
                  IconButton(
                    icon: FaIcon(
                      FontAwesomeIcons.floppyDisk,
                      size: 40,
                    ),
                    onPressed: () async {
                      String filepath =
                          await _saveFile(journeyHeader, exportType);
                      if (!context.mounted) return;
                      await showCommonDialog(context, filepath,
                          title: context.tr("journey.save_journey_data_title"),
                          confirmButtonText: context.tr("common.ok"));
                      if (!context.mounted) return;
                      Navigator.of(context).pop();
                    },
                  ),
                  Text(context.tr("journey.save_journey_data_title")),
                ],
              ),
              Column(
                mainAxisAlignment: MainAxisAlignment.center,
                mainAxisSize: MainAxisSize.min,
                children: [
                  IconButton(
                    icon: FaIcon(
                      FontAwesomeIcons.shareFromSquare,
                      size: 40,
                    ),
                    onPressed: () {
                      _share(journeyHeader, exportType);
                      Navigator.of(context).pop();
                    },
                  ),
                  Text(context.tr("journey.share_journey_data_title")),
                ],
              ),
            ],
          ),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    final journeyKindName = switch (widget.journeyHeader.journeyKind) {
      JourneyKind.defaultKind => context.tr("journey_kind.default"),
      JourneyKind.flight => context.tr("journey_kind.flight"),
    };
    return Scaffold(
      appBar: AppBar(
          title: Text(context.tr("journey.journey_info_bar_title")),
          actions: [
            PopupMenuButton<ExportType>(
              onSelected: (value) {
                if (Platform.isAndroid) {
                  _showDialog(context, widget.journeyHeader, value);
                } else if (Platform.isIOS) {
                  _share(widget.journeyHeader, value);
                }
              },
              itemBuilder: (BuildContext context) {
                return [
                  PopupMenuItem<ExportType>(
                    value: ExportType.mldx,
                    child: Text(context.tr("journey.export_mldx_data_menu")),
                  ),
                  PopupMenuItem<ExportType>(
                    value: ExportType.gpx,
                    child: Text(context.tr("journey.export_gpx_data_menu")),
                  ),
                  PopupMenuItem<ExportType>(
                    value: ExportType.kml,
                    child: Text(context.tr("journey.export_kml_data_menu")),
                  ),
                ];
              },
              icon: Icon(Icons.share),
            ),
            IconButton(
              onPressed: () async {
                await _editJourneyInfo(context);
              },
              icon: Icon(Icons.edit),
            ),
            IconButton(
              onPressed: () async {
                await _deleteJourneyInfo(context);
              },
              icon: Icon(Icons.delete),
            ),
          ]),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Text("Journey ID: ${widget.journeyHeader.id}"),
            Text(
                "Journey Date: ${naiveDateToString(date: widget.journeyHeader.journeyDate)}"),
            Text("JourneyKind: $journeyKindName"),
            Text(
                "Start Time: ${widget.journeyHeader.start != null ? fmt.format(widget.journeyHeader.start!.toLocal()) : ""}"),
            Text(
                "End Time: ${widget.journeyHeader.end != null ? fmt.format(widget.journeyHeader.end!.toLocal()) : ""}"),
            Text(
                "Created At: ${fmt.format(widget.journeyHeader.createdAt.toLocal())}"),
            Text("Revision: ${widget.journeyHeader.revision}"),
            Text("Note: ${widget.journeyHeader.note}"),
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
