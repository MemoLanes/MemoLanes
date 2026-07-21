import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/basic_bottom_sheet.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/capsule_style_bar_content.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:path_provider/path_provider.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

class JourneyInfoPage extends StatefulWidget {
  const JourneyInfoPage({
    super.key,
    required this.journeyHeader,
    this.previewJourneyData,
  });

  final JourneyHeader journeyHeader;
  final api.OpaqueJourneyData? previewJourneyData;

  @override
  State<JourneyInfoPage> createState() => _JourneyInfoPage();
}

class _JourneyInfoPage extends State<JourneyInfoPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  late JourneyHeader _journeyHeader;
  api.MapRendererProxy? _mapRendererProxy;
  MapBounds? _initialMapBounds;

  @override
  void initState() {
    super.initState();
    _journeyHeader = widget.journeyHeader;
    _refreshJourneyInfo();
  }

  bool get _isPreviewMode => widget.previewJourneyData != null;

  double _panelMaxHeight(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final baseMaxHeight = _isPreviewMode ? 400.0 : 480.0;
    final overlayBarHeight = mediaQuery.padding.top * 0.8 +
        CapsuleBarConstants.barContentHeight +
        CapsuleBarConstants.barBottomInset;
    final availableHeight = mediaQuery.size.height - overlayBarHeight;

    if (availableHeight < 120.0) {
      return 120.0;
    }
    return availableHeight < baseMaxHeight ? availableHeight : baseMaxHeight;
  }

  Future<void> _refreshJourneyInfo() async {
    final rendererAndBounds = widget.previewJourneyData != null
        ? await api.getMapRendererProxyForJourneyData(
            journeyData: widget.previewJourneyData!)
        : await api.getMapRendererProxyForJourney(journeyId: _journeyHeader.id);

    if (_isPreviewMode) {
      if (!mounted) return;
      setState(() {
        _mapRendererProxy = rendererAndBounds.$1;
        _initialMapBounds = rendererAndBounds.$2;
      });
      return;
    }

    final allJourneys = await api.listAllJourneys();
    final latestHeader = allJourneys
        .where((j) => j.id == _journeyHeader.id)
        .cast<JourneyHeader?>()
        .firstOrNull;

    if (!mounted) return;
    setState(() {
      _mapRendererProxy = rendererAndBounds.$1;
      _initialMapBounds = rendererAndBounds.$2;
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
      popCurrentRoute(context, true);
    }
  }

  Future<void> _editJourneyInfo(BuildContext context) async {
    var trackEdited = false;
    final result = await navigatorPush(
      context,
      page: Scaffold(
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
      ),
    );

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
    await navigatorPush(
      context,
      page: JourneyTrackEditPage(editSession: session),
    );
    await _refreshJourneyInfo();
  }

  Future<CommonExportResult> _generateExportFile(
      JourneyHeader journeyHeader, CommonExportFormat exportFormat) async {
    final tmpDir = await getTemporaryDirectory();
    final dateStr = naiveDateToString(date: journeyHeader.journeyDate);
    final filePath =
        "${tmpDir.path}/$dateStr-${journeyHeader.revision}.${exportFormat.extension}";
    final exportType = switch (exportFormat) {
      CommonExportFormat.mldx => api.ExportType.mldx,
      CommonExportFormat.fwss => api.ExportType.fwss,
      CommonExportFormat.gpx => api.ExportType.gpx,
      CommonExportFormat.kml => api.ExportType.kml,
    };

    final exportResult = await api.exportJourney(
        targetFilepath: filePath,
        journeyId: journeyHeader.id,
        exportType: exportType);
    return CommonExportResult.create(exportResult, filePath);
  }

  void _export() async {
    final supportsVectorExport =
        _journeyHeader.journeyType != JourneyType.bitmap;
    await showCommonExportWithFormatPicker(
      context: context,
      title: context.tr("data.export_data.export_journey_title"),
      formats: [
        CommonExportFormat.mldx,
        CommonExportFormat.fwss,
        if (supportsVectorExport) CommonExportFormat.kml,
        if (supportsVectorExport) CommonExportFormat.gpx,
      ],
      exportFile: (format) => _generateExportFile(_journeyHeader, format),
    );
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    final mapBoundsPadding =
        CapsuleStyleOverlayAppBar.mapFitPaddingForBottomOverlay(
      context,
      bottomOverlayHeight: _panelMaxHeight(context),
    );
    final journeyKindName = switch (_journeyHeader.journeyKind) {
      JourneyKind.defaultKind => context.tr("journey_kind.default"),
      JourneyKind.flight => context.tr("journey_kind.flight"),
    };
    return Scaffold(
      body: Stack(
        children: [
          SlidingUpPanel(
            color: Colors.black,
            borderRadius: BorderRadius.only(
              topLeft: Radius.circular(16.0),
              topRight: Radius.circular(16.0),
            ),
            maxHeight: _panelMaxHeight(context),
            minHeight: MediaQuery.sizeOf(context).width >
                    MediaQuery.sizeOf(context).height
                ? 32
                : 100,
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
                  Expanded(
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
                        SizedBox(height: 16.0),
                        if (!_isPreviewMode)
                          Row(
                            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                            children: [
                              ElevatedButton(
                                onPressed: _export,
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
            body: mapRendererProxy == null
                ? const CircularProgressIndicator()
                : BaseMapWebview(
                    key: const ValueKey("mapWidget"),
                    mapRendererProxy: mapRendererProxy,
                    initialMapBounds: _initialMapBounds,
                    initialMapBoundsPadding: mapBoundsPadding,
                  ),
          ),
          CapsuleStyleOverlayAppBar.overlayBar(
            title: context.tr("journey.journey_info_page_title"),
          ),
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
