import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_edit_form.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/index.dart';
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
  api.MapRendererProxy? _mapRendererProxy;
  MapView? _initialMapView;
  final PanelController _panelController = PanelController();

  bool _isEditMode = false;
  DateTime? _displayStart;
  DateTime? _displayEnd;
  NaiveDate? _displayJourneyDate;
  String? _displayNote;
  JourneyKind? _displayJourneyKind;

  DateTime? get _start => _displayStart ?? widget.journeyHeader.start;
  DateTime? get _end => _displayEnd ?? widget.journeyHeader.end;
  NaiveDate get _journeyDate =>
      _displayJourneyDate ?? widget.journeyHeader.journeyDate;
  String get _note => _displayNote ?? widget.journeyHeader.note ?? '';
  JourneyKind get _journeyKind =>
      _displayJourneyKind ?? widget.journeyHeader.journeyKind;

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

  void _enterEditMode() {
    setState(() => _isEditMode = true);
    _panelController.animatePanelToPosition(1.0);
  }

  void _exitEditMode() {
    setState(() => _isEditMode = false);
    _panelController.close();
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
    String filePath =
        await _generateExportFile(widget.journeyHeader, exportType);
    if (!mounted) return;
    await showCommonExport(context, filePath, deleteFile: true);
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    final journeyKindName = switch (_journeyKind) {
      JourneyKind.defaultKind => context.tr("journey_kind.default"),
      JourneyKind.flight => context.tr("journey_kind.flight"),
    };
    return Scaffold(
      body: Stack(
        children: [
          SlidingUpPanel(
        controller: _panelController,
        color: Colors.black,
        borderRadius: BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
        maxHeight: 480,
        defaultPanelState: PanelState.CLOSED,
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
                  padding: EdgeInsets.symmetric(vertical: 16.0),
                  children: [
                    if (_isEditMode)
                      JourneyInfoEditForm(
                        initialStartTime: _start,
                        initialEndTime: _end,
                        initialJourneyDate: _journeyDate,
                        initialNote: _note,
                        initialJourneyKind: _journeyKind,
                        onSave: (JourneyInfo info) async {
                          await api.updateJourneyMetadata(
                            id: widget.journeyHeader.id,
                            journeyInfo: info,
                          );
                          if (!mounted) return;
                          setState(() {
                            _displayStart = info.startTime;
                            _displayEnd = info.endTime;
                            _displayJourneyDate = info.journeyDate;
                            _displayNote = info.note;
                            _displayJourneyKind = info.journeyKind;
                            _isEditMode = false;
                          });
                          _panelController.close();
                        },
                        onCancel: _exitEditMode,
                        showCancelButton: true,
                      )
                    else
                      _buildViewContent(context, journeyKindName),
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
            moreMenuContent: _JourneyMoreMenuContent(
              isEditMode: _isEditMode,
              onExport: () => _showExportDataCard(context, widget.journeyHeader.journeyType),
              onEdit: () => _enterEditMode(),
              onCancelEdit: () => _exitEditMode(),
              onDelete: () => _deleteJourneyInfo(context),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildViewContent(BuildContext context, String journeyKindName) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        LabelTile(
          label: context.tr("journey.journey_date"),
          position: LabelTilePosition.top,
          trailing: LabelTileContent(
            content: naiveDateToString(date: _journeyDate),
          ),
        ),
        LabelTile(
          label: context.tr("journey.journey_kind"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(content: journeyKindName),
        ),
        LabelTile(
          label: context.tr("journey.start_time"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(
            content: _start != null ? fmt.format(_start!.toLocal()) : "",
          ),
        ),
        LabelTile(
          label: context.tr("journey.end_time"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(
            content: _end != null ? fmt.format(_end!.toLocal()) : "",
          ),
        ),
        LabelTile(
          label: context.tr("journey.created_at"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(
            content: fmt.format(widget.journeyHeader.createdAt.toLocal()),
          ),
        ),
        LabelTile(
          label: context.tr("journey.note"),
          position: LabelTilePosition.bottom,
          maxHeight: 150,
          trailing: Padding(
            padding: EdgeInsets.symmetric(vertical: 8.0),
            child: LabelTileContent(
              content: _note,
              contentMaxLines: 5,
            ),
          ),
        ),
      ],
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
}

class _JourneyMoreMenuContent extends StatelessWidget {
  const _JourneyMoreMenuContent({
    required this.isEditMode,
    required this.onExport,
    required this.onEdit,
    required this.onCancelEdit,
    required this.onDelete,
  });

  final bool isEditMode;
  final VoidCallback onExport;
  final VoidCallback onEdit;
  final VoidCallback onCancelEdit;
  final VoidCallback onDelete;

  static const Color _textColor = Color(0xFFE5E5E7);

  @override
  Widget build(BuildContext context) {
    return IntrinsicWidth(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _menuTile(
            context,
            context.tr("common.export"),
            onExport,
          ),
          _menuTile(
            context,
            isEditMode ? context.tr("common.cancel") : context.tr("common.edit"),
            isEditMode ? onCancelEdit : onEdit,
          ),
          _menuTile(
            context,
            context.tr("common.delete"),
            onDelete,
          ),
        ],
      ),
    );
  }

  Widget _menuTile(
    BuildContext context,
    String label,
    VoidCallback onTap,
  ) {
    return InkWell(
      onTap: () {
        Navigator.pop(context);
        onTap();
      },
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 12, horizontal: 16),
        child: Text(
          label,
          style: const TextStyle(color: _textColor, fontSize: 14),
        ),
      ),
    );
  }
}
