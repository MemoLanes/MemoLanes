import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/body/journey/journey_export.dart';
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/body/journey/journey_track_edit_page.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/capsule_style_bar_content.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/journey_header.dart';
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
    final overlayBarHeight =
        mediaQuery.padding.top * 0.8 +
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
            journeyData: widget.previewJourneyData!,
          )
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
      context,
      context.tr("journey.delete_journey_message"),
      hasCancel: true,
      title: context.tr("journey.delete_journey_title"),
      confirmButtonText: context.tr("common.delete"),
      confirmVariant: AppButtonVariant.danger,
    )) {
      await api.deleteJourney(journeyId: _journeyHeader.id);
      if (!context.mounted) return;
      popCurrentRoute(context, true);
    }
  }

  Future<void> _editJourneyInfo(BuildContext context) async {
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
            journeyDate: _journeyHeader.journeyDate.toSimpleDate(),
            note: _journeyHeader.note,
            journeyKind: _journeyHeader.journeyKind,
            saveData: (JourneyInfo journeyInfo, _) async {
              await api.updateJourneyMetadata(
                id: _journeyHeader.id,
                journeyInfo: journeyInfo,
              );
            },
          ),
        ),
      ),
    );

    // `JourneyInfoEditPage` pops with `true` when metadata is saved.
    if (result == true && mounted) {
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
    if (!mounted) return;
    await _refreshJourneyInfo();
  }

  Future<void> _export() async {
    await showJourneyExportPicker(context, _journeyHeader);
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    if (_isPreviewMode) {
      return _buildImportPreview(context, mapRendererProxy);
    }

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
            color: StyleConstants.canvasColor,
            borderRadius: BorderRadius.only(
              topLeft: Radius.circular(16.0),
              topRight: Radius.circular(16.0),
            ),
            maxHeight: _panelMaxHeight(context),
            minHeight:
                MediaQuery.sizeOf(context).width >
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
                          color: StyleConstants.mutedInkColor.withValues(
                            alpha: 0.44,
                          ),
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
                            content: _journeyHeader.journeyDate
                                .toSimpleDate()
                                .toString(),
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
                            content: fmt.format(
                              _journeyHeader.createdAt.toLocal(),
                            ),
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
                              SizedBox(
                                width: 100,
                                child: AppButton(
                                  label: context.tr("common.export"),
                                  icon: Icons.ios_share_rounded,
                                  variant: AppButtonVariant.secondary,
                                  size: AppButtonSize.compact,
                                  onPressed: _export,
                                ),
                              ),
                              SizedBox(
                                width: 100,
                                child: AppButton(
                                  label: context.tr("common.edit"),
                                  icon: Icons.edit_outlined,
                                  variant: AppButtonVariant.primary,
                                  size: AppButtonSize.compact,
                                  onPressed: () async => _showEditMenu(context),
                                ),
                              ),
                              SizedBox(
                                width: 100,
                                child: AppButton(
                                  label: context.tr("common.delete"),
                                  icon: Icons.delete_outline_rounded,
                                  variant: AppButtonVariant.danger,
                                  size: AppButtonSize.compact,
                                  onPressed: () async =>
                                      await _deleteJourneyInfo(context),
                                ),
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

  Widget _buildImportPreview(
    BuildContext context,
    api.MapRendererProxy? mapRendererProxy,
  ) {
    final viewPadding = MediaQuery.viewPaddingOf(context);
    final mapPadding = EdgeInsets.fromLTRB(
      24,
      viewPadding.top + 82,
      24,
      viewPadding.bottom + 270,
    );

    return Scaffold(
      backgroundColor: StyleConstants.canvasColor,
      body: Stack(
        fit: StackFit.expand,
        children: [
          if (mapRendererProxy == null)
            const Center(child: CircularProgressIndicator())
          else
            BaseMapWebview(
              key: const ValueKey('importJourneyPreviewMap'),
              mapRendererProxy: mapRendererProxy,
              initialMapBounds: _initialMapBounds,
              initialMapBoundsPadding: mapPadding,
            ),
          Positioned(
            left: viewPadding.left + 16,
            right: viewPadding.right + 16,
            bottom: viewPadding.bottom + 16,
            child: Align(
              alignment: Alignment.bottomCenter,
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 430),
                child: PointerInterceptor(
                  child: ReadOnlyJourneyInfoCard(journey: _journeyHeader),
                ),
              ),
            ),
          ),
          Positioned(
            left: viewPadding.left + 16,
            top: viewPadding.top + 14,
            child: PointerInterceptor(
              child: MapGlassBackButton(
                onPressed: () => Navigator.maybePop(context),
              ),
            ),
          ),
        ],
      ),
    );
  }

  void _showEditMenu(BuildContext context) {
    showBasicCard(
      context,
      title: context.tr("common.edit"),
      builder: (dialogContext) => Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          AppOptionTile(
            icon: Icons.description_outlined,
            title: context.tr("journey.journey_info_edit_page_title"),
            onTap: () {
              Navigator.of(dialogContext).pop();
              _editJourneyInfo(context);
            },
          ),
          const SizedBox(height: 8),
          AppOptionTile(
            icon: Icons.edit_road_rounded,
            title: context.tr("journey.editor.page_title"),
            onTap: () async {
              Navigator.of(dialogContext).pop();
              _trackEdit(context);
            },
          ),
        ],
      ),
    );
  }
}
