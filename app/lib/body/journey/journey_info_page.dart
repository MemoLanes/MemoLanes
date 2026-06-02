import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart' show MapView;
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/map_panel_page.dart';
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

enum ExportType { mldx, kml, gpx }

enum _JourneyInfoPanelMode { info, edit }

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
  MapView? _initialMapView;
  _JourneyInfoPanelMode _panelMode = _JourneyInfoPanelMode.info;
  bool _journeyInfoChanged = false;

  @override
  void initState() {
    super.initState();
    _journeyHeader = widget.journeyHeader;
    _refreshJourneyInfo();
  }

  bool get _isPreviewMode => widget.previewJourneyData != null;

  Future<void> _refreshJourneyInfo() async {
    final mapRendererProxyAndCameraOption = widget.previewJourneyData != null
        ? await api.getMapRendererProxyForJourneyData(
            journeyData: widget.previewJourneyData!)
        : await api.getMapRendererProxyForJourney(journeyId: _journeyHeader.id);

    if (_isPreviewMode) {
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

  void _editJourneyInfo() {
    setState(() {
      _panelMode = _JourneyInfoPanelMode.edit;
    });
  }

  Future<bool> _saveJourneyInfo(JourneyInfo journeyInfo) async {
    await api.updateJourneyMetadata(
      id: _journeyHeader.id,
      journeyInfo: journeyInfo,
    );
    _journeyInfoChanged = true;
    await _refreshJourneyInfo();
    return true;
  }

  void _handleBack() {
    if (_panelMode == _JourneyInfoPanelMode.edit) {
      setState(() {
        _panelMode = _JourneyInfoPanelMode.info;
      });
      return;
    }
    Navigator.pop(context, _journeyInfoChanged ? true : null);
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

  Future<String> _generateExportFile(
      JourneyHeader journeyHeader, ExportType exportType) async {
    final tmpDir = await getTemporaryDirectory();
    final dateStr = naiveDateToString(date: journeyHeader.journeyDate);
    final filepath =
        "${tmpDir.path}/$dateStr-${journeyHeader.revision}.${exportType.name}";
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
    final isEditing = _panelMode == _JourneyInfoPanelMode.edit;

    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) {
        if (didPop) return;
        _handleBack();
      },
      child: MapPanelPage(
        title: isEditing
            ? context.tr("journey.journey_info_edit_page_title")
            : context.tr("journey.journey_info_page_title"),
        mapRendererProxy: mapRendererProxy,
        initialMapView: _initialMapView,
        maxHeight: isEditing ? 440 : (_isPreviewMode ? null : 480),
        expandPanel: true,
        loadingBody: const Center(child: CircularProgressIndicator()),
        onBack: _handleBack,
        panel: isEditing
            ? _buildEditPanel(context)
            : _buildInfoPanel(context, journeyKindName),
      ),
    );
  }

  Widget _buildEditPanel(BuildContext context) {
    return JourneyInfoEditPage(
      startTime: _journeyHeader.start,
      endTime: _journeyHeader.end,
      journeyDate: _journeyHeader.journeyDate,
      note: _journeyHeader.note,
      journeyKind: _journeyHeader.journeyKind,
      onSave: (journeyInfo, _) => _saveJourneyInfo(journeyInfo),
      popOnSave: false,
      onSaved: () {
        if (!mounted) return;
        setState(() {
          _panelMode = _JourneyInfoPanelMode.info;
        });
      },
    );
  }

  Widget _buildInfoPanel(BuildContext context, String journeyKindName) {
    return MlSingleChildScrollView(
      padding: const EdgeInsets.only(bottom: 16.0),
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
            content: fmt.format(_journeyHeader.createdAt.toLocal()),
          ),
        ),
        LabelTile(
          label: context.tr("journey.note"),
          position: LabelTilePosition.bottom,
          maxHeight: 150,
          trailing: Padding(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: LabelTileContent(
              content: _journeyHeader.note ?? "",
              contentMaxLines: 5,
            ),
          ),
        ),
        if (!_isPreviewMode) ...[
          const SizedBox(height: 8.0),
          _buildActionSection(context),
        ],
      ],
    );
  }

  Widget _buildActionSection(BuildContext context) {
    const gap = 6.0;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16.0),
      child: Row(
        children: [
          Expanded(
            child: _buildActionTile(
              context,
              icon: Icons.share,
              label: context.tr("common.export"),
              onTap: () => _showExportDataCard(
                context,
                _journeyHeader.journeyType,
              ),
            ),
          ),
          SizedBox(width: gap),
          Expanded(
            child: _buildActionTile(
              context,
              icon: Icons.edit,
              label: context.tr("journey.journey_info_edit_page_title"),
              onTap: _editJourneyInfo,
            ),
          ),
          SizedBox(width: gap),
          Expanded(
            child: _buildActionTile(
              context,
              icon: Icons.timeline,
              label: context.tr("journey.editor.page_title"),
              onTap: () => _trackEdit(context),
            ),
          ),
          SizedBox(width: gap),
          Expanded(
            child: _buildActionTile(
              context,
              icon: Icons.more_horiz,
              label: context.tr("common.more"),
              onTap: () => _showMoreActionCard(context),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildActionTile(
    BuildContext context, {
    required IconData icon,
    required String label,
    required VoidCallback onTap,
  }) {
    return SizedBox(
      height: 64,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(16.0),
          child: Ink(
            decoration: BoxDecoration(
              color: const Color(0x1AFFFFFF),
              borderRadius: BorderRadius.circular(16.0),
            ),
            padding: const EdgeInsets.symmetric(horizontal: 6.0, vertical: 7.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(icon, size: 22.0, color: Colors.white),
                const SizedBox(height: 4.0),
                Flexible(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 12.0,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _copyJourneyInfo(BuildContext context) async {
    final result = await navigatorPush(
      context,
      page: MapPanelPage(
        title: context.tr("journey.copy_journey"),
        mapRendererProxy: _mapRendererProxy,
        initialMapView: _initialMapView,
        maxHeight: 440,
        expandPanel: true,
        panel: JourneyInfoEditPage(
          startTime: _journeyHeader.start,
          endTime: _journeyHeader.end,
          journeyDate: _journeyHeader.journeyDate,
          note: _journeyHeader.note,
          journeyKind: _journeyHeader.journeyKind,
          onSave: (JourneyInfo journeyInfo, _) async {
            await showLoadingDialog<String>(
              asyncTask: api.copyJourney(
                journeyId: _journeyHeader.id,
                journeyInfo: journeyInfo,
              ),
            );
            return true;
          },
        ),
      ),
    );

    if (result != true || !context.mounted) return;
    Navigator.pop(context, true);
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

  void _showMoreActionCard(BuildContext context) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("journey.copy_journey"),
            icon: Icons.copy,
            onTap: () {
              _copyJourneyInfo(context);
            },
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("journey.delete_journey_title"),
            color: const Color(0xFFEC4162),
            icon: Icons.delete,
            onTap: () async {
              await _deleteJourneyInfo(context);
            },
          ),
        ],
      ),
    );
  }
}
