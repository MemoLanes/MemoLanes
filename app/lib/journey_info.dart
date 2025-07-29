import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/component/base_map_webview.dart';
import 'package:memolanes/component/cards/line_painter.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';
import 'package:memolanes/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/component/tiles/label_tile.dart';
import 'package:memolanes/component/tiles/label_tile_content.dart';
import 'package:memolanes/journey_edit.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils.dart';
import 'package:path_provider/path_provider.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

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
        body: SafeAreaWrapper(
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
      body: SlidingUpPanel(
        color: Colors.black,
        borderRadius: BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
        maxHeight: 480,
        defaultPanelState: PanelState.OPEN,
        panel: PointerInterceptor(
          child: SafeAreaWrapper(
            child: Column(
              children: [
                Padding(
                  padding: EdgeInsets.only(top: 12.0),
                  child: Center(
                    child: CustomPaint(
                      size: Size(40.0, 4.0),
                      painter: LinePainter(
                        color: const Color(0xFFB5B5B5),
                      ),
                    ),
                  ),
                ),
                SizedBox(height: 16.0),
                SizedBox(
                  height: 340,
                  child: MlSingleChildScrollView(
                    children: [
                      LabelTile(
                        label: context.tr("journey.journey_date"),
                        position: LabelTilePosition.top,
                        trailing: LabelTileContent(
                          content: naiveDateToString(
                            date: widget.journeyHeader.journeyDate,
                          ),
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.journey_kind"),
                        position: LabelTilePosition.middle,
                        trailing: LabelTileContent(
                          content: journeyKindName,
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.start_time"),
                        position: LabelTilePosition.middle,
                        trailing: LabelTileContent(
                          content: widget.journeyHeader.start != null
                              ? fmt
                                  .format(widget.journeyHeader.start!.toLocal())
                              : "",
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.end_time"),
                        position: LabelTilePosition.middle,
                        trailing: LabelTileContent(
                          content: widget.journeyHeader.end != null
                              ? fmt.format(widget.journeyHeader.end!.toLocal())
                              : "",
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.created_at"),
                        position: LabelTilePosition.middle,
                        trailing: LabelTileContent(
                          content: fmt
                              .format(widget.journeyHeader.createdAt.toLocal()),
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.note"),
                        position: LabelTilePosition.bottom,
                        maxHeight: 150,
                        trailing: Padding(
                          padding: EdgeInsets.symmetric(vertical: 8.0),
                          child: LabelTileContent(
                            content: widget.journeyHeader.note ?? "",
                            contentMaxLines: 5,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                SizedBox(height: 16.0),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                  children: [
                    ElevatedButton(
                      onPressed: () => showExportDataCard(
                        context,
                        journeyType: widget.journeyHeader.journeyType,
                        onLabelTaped: (exportType) async {
                          _export(exportType);
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
                      onPressed: () async => await _deleteJourneyInfo(context),
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
        ),
        body: mapRendererProxy == null
            ? const CircularProgressIndicator()
            : BaseMapWebview(
                key: const ValueKey("mapWidget"),
                mapRendererProxy: mapRendererProxy,
                initialMapView: _initialMapView,
              ),
      ),
    );
  }
}
