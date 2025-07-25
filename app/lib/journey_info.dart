import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/component/base_map_webview.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';
import 'package:memolanes/journey_edit.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils.dart';
import 'package:path_provider/path_provider.dart';

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
  MapView? _initialMapView;

  @override
  void initState() {
    super.initState();
    api
        .getMapRendererProxyForJourney(journeyId: widget.journeyHeader.id)
        .then((mapRendererProxyAndCameraOption) {
      setState(() {
        _mapRendererProxy = mapRendererProxyAndCameraOption.$1;
        final cameraOption = mapRendererProxyAndCameraOption.$2;
        if (cameraOption != null) {
          _initialMapView = (
            lng: cameraOption.lng,
            lat: cameraOption.lat,
            zoom: cameraOption.zoom,
          );
        }
      });
    });
  }

  Future<void> _deleteJourneyInfo(BuildContext context) async {
    if (await showCommonDialog(
        context, context.tr("journey.delete_journey_message"),
        hasCancel: true,
        title: context.tr("journey.delete_journey_title"),
        confirmButtonText: context.tr("common.delete"),
        confirmGroundColor: Colors.red,
        confirmTextColor: Colors.white)) {
      await api.deleteJourney(journeyId: widget.journeyHeader.id);
      if (!context.mounted) return;
      Navigator.pop(context, true);
    }
  }

  Future<void> _editJourneyInfo(BuildContext context) async {
    final result =
        await Navigator.push(context, MaterialPageRoute(builder: (context) {
      return Scaffold(
        appBar: AppBar(
          title: Text(context.tr("journey.journey_info_edit_page_title")),
        ),
        body: Center(
          child: JourneyInfoEditor(
            startTime: widget.journeyHeader.start,
            endTime: widget.journeyHeader.end,
            journeyDate: widget.journeyHeader.journeyDate,
            note: widget.journeyHeader.note,
            journeyKind: widget.journeyHeader.journeyKind,
            saveData: (JourneyInfo journeyInfo) async {
              await api.updateJourneyMetadata(
                  id: widget.journeyHeader.id, journeyinfo: journeyInfo);
            },
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

  Future<String> _generateExportFile(
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

  void _export(ExportType exportType) async {
    String filePath =
        await _generateExportFile(widget.journeyHeader, exportType);
    if (!mounted) return;
    await showCommonExport(context, filePath, deleteFile: true);
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
        title: Text(context.tr("journey.journey_info_page_title")),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Expanded(
              child: mapRendererProxy == null
                  ? (const CircularProgressIndicator())
                  : (BaseMapWebview(
                      key: const ValueKey("mapWidget"),
                      mapRendererProxy: mapRendererProxy,
                      initialMapView: _initialMapView,
                    )),
            ),
            SafeAreaWrapper(
              child: Column(
                children: [
                  SizedBox(height: 16.0),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.journey_id")}:'),
                      Text(widget.journeyHeader.id),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.journey_date")}:'),
                      Text(naiveDateToString(
                          date: widget.journeyHeader.journeyDate)),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.journey_kind")}:'),
                      Text(journeyKindName),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.start_time")}:'),
                      Text(widget.journeyHeader.start != null
                          ? fmt.format(widget.journeyHeader.start!.toLocal())
                          : ""),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.end_time")}:'),
                      Text(widget.journeyHeader.end != null
                          ? fmt.format(widget.journeyHeader.end!.toLocal())
                          : ""),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.created_at")}:'),
                      Text(
                          fmt.format(widget.journeyHeader.createdAt.toLocal())),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.revision")}:'),
                      Text(widget.journeyHeader.revision),
                    ],
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text('${context.tr("journey.note")}:'),
                      Text(widget.journeyHeader.note ?? ''),
                    ],
                  ),
                  SizedBox(height: 16.0),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                    children: [
                      ElevatedButton(
                        onPressed: () => showExportDataCard(
                          context,
                          onLabelTaped: (name) async {
                            switch (name) {
                              case 'MLDX':
                                _export(ExportType.mldx);
                                break;
                              case 'KML':
                                _export(ExportType.kml);
                                break;
                              case 'GPX':
                                _export(ExportType.gpx);
                                break;
                            }
                          },
                        ),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: const Color(0xFFFFFFFF),
                          foregroundColor: Colors.black,
                          fixedSize: Size(100, 42),
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(25.0),
                          ),
                        ),
                        child: Text(context.tr("common.export")),
                      ),
                      ElevatedButton(
                        onPressed: () async => await _editJourneyInfo(context),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: const Color(0xFFB6E13D),
                          foregroundColor: Colors.black,
                          fixedSize: Size(100, 42),
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(25.0),
                          ),
                        ),
                        child: Text(context.tr("common.edit")),
                      ),
                      ElevatedButton(
                        onPressed: () async =>
                            await _deleteJourneyInfo(context),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: const Color(0xFFEC4162),
                          foregroundColor: Colors.black,
                          fixedSize: Size(100, 42),
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(25.0),
                          ),
                        ),
                        child: Text(context.tr("common.delete")),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
