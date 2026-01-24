import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
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
  late JourneyHeader _journeyHeader;
  api.MapRendererProxy? _mapRendererProxy;
  MapView? _initialMapView;

  @override
  void initState() {
    super.initState();
    _journeyHeader = widget.journeyHeader;
    _refreshJourneyInfo();
  }

  Future<void> _refreshJourneyInfo() async {
    final mapRendererProxyAndCameraOption =
        await api.getMapRendererProxyForJourney(journeyId: _journeyHeader.id);

    JourneyHeader? latestHeader;
    try {
      final allJourneys = await api.listAllJourneys();
      latestHeader = allJourneys
          .where((j) => j.id == _journeyHeader.id)
          .cast<JourneyHeader?>()
          .firstOrNull;
    } catch (_) {
      // Best-effort refresh; map renderer proxy is the important part for track changes.
    }

    if (!mounted) return;
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
      if (latestHeader != null) {
        _journeyHeader = latestHeader;
      }
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
      await api.deleteJourney(journeyId: _journeyHeader.id);
      if (!context.mounted) return;
      Navigator.pop(context, true);
    }
  }

  Future<void> _editJourneyInfo(BuildContext context) async {
    var trackEdited = false;
    final result =
        await Navigator.push(context, MaterialPageRoute(builder: (context) {
      return Scaffold(
        appBar: AppBar(
          title: Text(context.tr("journey.journey_info_edit_page_title")),
        ),
        body: SafeAreaWrapper(
          child: JourneyInfoEditPage(
            startTime: _journeyHeader.start,
            endTime: _journeyHeader.end,
            journeyDate: _journeyHeader.journeyDate,
            note: _journeyHeader.note,
            journeyKind: _journeyHeader.journeyKind,
            saveData: (JourneyInfo journeyInfo) async {
              await api.updateJourneyMetadata(
                  id: _journeyHeader.id, journeyInfo: journeyInfo);
            },
          ),
        ),
      );
    }));

    // `JourneyInfoEditPage` pops with `true` when metadata is saved.
    if (result == true || trackEdited) {
      await _refreshJourneyInfo();
    }
  }

  Future<void> _trackEdit(BuildContext context) async {
    await Navigator.push(context, MaterialPageRoute(builder: (context) {
      return Scaffold(
        appBar: AppBar(
          title: Text(context.tr("journey.journey_track_edit_title")),
        ),
        body: JourneyTrackEditPage(
          journeyId: _journeyHeader.id,
        ),
      );
    }));
  }

  Future<String> _generateExportFile(
      JourneyHeader journeyHeader, ExportType exportType) async {
    final tmpDir = await getTemporaryDirectory();
    final dateStr = naiveDateToString(date: journeyHeader.journeyDate);
    final filepath =
        "${tmpDir.path}/${dateStr}-${journeyHeader.revision}.${exportType.name}";
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
    String filePath = await _generateExportFile(_journeyHeader, exportType);
    if (!mounted) return;
    await showCommonExport(context, filePath, deleteFile: true);
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    final journeyKindName = switch (_journeyHeader.journeyKind) {
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
                            date: _journeyHeader.journeyDate,
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
                          content: _journeyHeader.start != null
                              ? fmt.format(_journeyHeader.start!.toLocal())
                              : "",
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.end_time"),
                        position: LabelTilePosition.middle,
                        trailing: LabelTileContent(
                          content: _journeyHeader.end != null
                              ? fmt.format(_journeyHeader.end!.toLocal())
                              : "",
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.created_at"),
                        position: LabelTilePosition.middle,
                        trailing: LabelTileContent(
                          content:
                              fmt.format(_journeyHeader.createdAt.toLocal()),
                        ),
                      ),
                      LabelTile(
                        label: context.tr("journey.note"),
                        position: LabelTilePosition.bottom,
                        maxHeight: 150,
                        trailing: Padding(
                          padding: EdgeInsets.symmetric(vertical: 8.0),
                          child: LabelTileContent(
                            content: _journeyHeader.note ?? "",
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
                      onPressed: () => _showExportDataCard(
                        context,
                        _journeyHeader.journeyType,
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
                      onPressed: () async => _showEditMenu(context),
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

  void _showExportDataCard(BuildContext context, JourneyType journeyType) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: journeyType != JourneyType.bitmap
                ? CardLabelTilePosition.top
                : CardLabelTilePosition.single,
            label: context.tr("journey.export_journey_as_mldx"),
            onTap: () {
              _export(ExportType.mldx);
            },
            top: false,
          ),
          if (journeyType != JourneyType.bitmap) ...[
            CardLabelTile(
              position: CardLabelTilePosition.middle,
              label: context.tr("journey.export_journey_as_kml"),
              onTap: () {
                _export(ExportType.kml);
              },
            ),
            CardLabelTile(
              position: CardLabelTilePosition.bottom,
              label: context.tr("journey.export_journey_as_gpx"),
              onTap: () {
                _export(ExportType.gpx);
              },
            ),
          ]
        ],
      ),
    );
  }

  void _showEditMenu(BuildContext context) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("journey.journey_info_edit_page_title"),
            onTap: () {
              _editJourneyInfo(context);
            },
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("journey.journey_track_edit_title"),
            onTap: () async {
              _trackEdit(context);
            },
          ),
        ],
      ),
    );
  }
}
