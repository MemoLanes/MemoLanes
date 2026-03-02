import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:path_provider/path_provider.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

enum ExportType { mldx, kml, gpx, rawDataCsv, rawDataGpx }

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
  bool? _hasRawData;

  @override
  void initState() {
    super.initState();
    _journeyHeader = widget.journeyHeader;
    _refreshJourneyInfo();
  }

  Future<void> _refreshJourneyInfo() async {
    final mapRendererProxyAndCameraOption =
        await api.getMapRendererProxyForJourney(journeyId: _journeyHeader.id);

    final allJourneys = await api.listAllJourneys();
    final latestHeader = allJourneys
        .where((j) => j.id == _journeyHeader.id)
        .cast<JourneyHeader?>()
        .firstOrNull;

    final hasRawData =
        await api.hasJourneyRawData(journeyId: _journeyHeader.id);

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
      _hasRawData = hasRawData;
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
        appBar: CapsuleStyleAppBar(
          title: context.tr("journey.journey_info_edit_page_title"),
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
    final session = await EditSession.newInstance(journeyId: _journeyHeader.id);
    if (!context.mounted) return;
    if (session == null) {
      await showCommonDialog(
        context,
        context.tr("journey.editor.bitmap_not_supported"),
      );
      return;
    }
    await Navigator.push(context, MaterialPageRoute(builder: (context) {
      return JourneyTrackEditPage(editSession: session);
    }));
    await _refreshJourneyInfo();
  }

  Future<String> _generateExportFile(
      JourneyHeader journeyHeader, ExportType exportType) async {
    final tmpDir = await getTemporaryDirectory();
    final dateStr = naiveDateToString(date: journeyHeader.journeyDate);
    final isRaw = exportType == ExportType.rawDataCsv ||
        exportType == ExportType.rawDataGpx;
    final suffix = isRaw ? '-raw' : '';
    final ext = switch (exportType) {
      ExportType.mldx => 'mldx',
      ExportType.kml => 'kml',
      ExportType.gpx => 'gpx',
      ExportType.rawDataCsv => 'csv',
      ExportType.rawDataGpx => 'gpx',
    };
    final filepath =
        "${tmpDir.path}/$dateStr-${journeyHeader.revision}$suffix.$ext";
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
      case ExportType.rawDataCsv:
        await api.exportJourneyRawDataCsv(
            journeyId: journeyHeader.id, targetFilepath: filepath);
        break;
      case ExportType.rawDataGpx:
        await api.exportJourneyRawDataGpx(
            journeyId: journeyHeader.id, targetFilepath: filepath);
        break;
    }
    return filepath;
  }

  void _export(ExportType exportType) async {
    String filePath = await _generateExportFile(_journeyHeader, exportType);
    if (!mounted) return;
    await showCommonExport(context, filePath, deleteFile: true);
  }

  Widget _buildActionButton({
    required String label,
    required Color backgroundColor,
    required VoidCallback onPressed,
  }) {
    return ElevatedButton(
      onPressed: onPressed,
      style: ElevatedButton.styleFrom(
        backgroundColor: backgroundColor,
        foregroundColor: Colors.black,
        minimumSize: Size(88, 42),
        padding: EdgeInsets.symmetric(horizontal: 16.0),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(25.0),
        ),
      ),
      child: Text(label),
    );
  }

  void _showExportOptionCard(
    BuildContext context,
    List<({String label, ExportType type})> options,
  ) {
    if (options.isEmpty) return;
    final tiles = <CardLabelTile>[];
    for (var i = 0; i < options.length; i++) {
      final option = options[i];
      final position = options.length == 1
          ? CardLabelTilePosition.single
          : i == 0
              ? CardLabelTilePosition.top
              : i == options.length - 1
                  ? CardLabelTilePosition.bottom
                  : CardLabelTilePosition.middle;
      tiles.add(CardLabelTile(
        position: position,
        label: option.label,
        onTap: () => _export(option.type),
        top: i != 0,
      ));
    }
    showBasicCard(
      context,
      child: OptionCard(
        children: tiles,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    final journeyKindName = switch (_journeyHeader.journeyKind) {
      JourneyKind.defaultKind => context.tr("journey_kind.default"),
      JourneyKind.flight => context.tr("journey_kind.flight"),
    };
    final isSingleRowButtons = _hasRawData != true;
    final panelMaxHeight = isSingleRowButtons ? 420.0 : 480.0;
    final infoAreaHeight = isSingleRowButtons ? 334.0 : 340.0;
    return Scaffold(
      body: Stack(
        children: [
          SlidingUpPanel(
            color: Colors.black,
            borderRadius: BorderRadius.only(
              topLeft: Radius.circular(16.0),
              topRight: Radius.circular(16.0),
            ),
            maxHeight: panelMaxHeight,
            defaultPanelState: PanelState.OPEN,
            panel: PointerInterceptor(
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
                    height: infoAreaHeight,
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
                  Padding(
                    padding: EdgeInsets.symmetric(horizontal: 16.0),
                    child: Wrap(
                      alignment: WrapAlignment.center,
                      spacing: 10.0,
                      runSpacing: 10.0,
                      children: [
                        _buildActionButton(
                          label: context.tr("common.export"),
                          backgroundColor: const Color(0xFFFFFFFF),
                          onPressed: () => _showExportDataCard(
                            context,
                            _journeyHeader.journeyType,
                          ),
                        ),
                        _buildActionButton(
                          label: context.tr("common.edit"),
                          backgroundColor: const Color(0xFFB6E13D),
                          onPressed: () async => _showEditMenu(context),
                        ),
                        _buildActionButton(
                          label: context.tr("common.delete"),
                          backgroundColor: const Color(0xFFEC4162),
                          onPressed: () async =>
                              await _deleteJourneyInfo(context),
                        ),
                        if (_hasRawData == true)
                          _buildActionButton(
                            label: context.tr("journey.export_raw_data"),
                            backgroundColor: const Color(0xFFE8F4CC),
                            onPressed: () => _showExportRawDataChoice(context),
                          ),
                      ],
                    ),
                  ),
                ],
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
          CapsuleStyleOverlayAppBar.overlayBar(
            title: context.tr("journey.journey_info_page_title"),
          ),
        ],
      ),
    );
  }

  void _showExportDataCard(BuildContext context, JourneyType journeyType) {
    final options = <({String label, ExportType type})>[
      (
        label: context.tr("journey.export_journey_as_mldx"),
        type: ExportType.mldx,
      ),
    ];
    if (journeyType != JourneyType.bitmap) {
      options.add((
        label: context.tr("journey.export_journey_as_kml"),
        type: ExportType.kml,
      ));
      options.add((
        label: context.tr("journey.export_journey_as_gpx"),
        type: ExportType.gpx,
      ));
    }
    _showExportOptionCard(context, options);
  }

  void _showExportRawDataChoice(BuildContext context) {
    _showExportOptionCard(context, [
      (
        label: context.tr("journey.export_raw_data_csv"),
        type: ExportType.rawDataCsv,
      ),
      (
        label: context.tr("journey.export_raw_data_gpx"),
        type: ExportType.rawDataGpx,
      ),
    ]);
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
            label: context.tr("journey.editor.page_title"),
            onTap: () async {
              _trackEdit(context);
            },
          ),
        ],
      ),
    );
  }
}
