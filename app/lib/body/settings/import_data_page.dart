import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fpdart/fpdart.dart' as f;
import 'package:memolanes/body/journey/journey_info_edit_page.dart';
import 'package:memolanes/body/settings/import_preprocessor_notice.dart';
import 'package:memolanes/body/settings/vector_multi_import_page.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/import_loading_page.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

class ImportDataPage extends StatefulWidget {
  const ImportDataPage({
    super.key,
    required this.path,
    required this.importType,
  });

  final String path;
  final ImportType importType;

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

enum ImportType { fow, vector }

class _ImportDataPage extends State<ImportDataPage> {
  import_api.JourneyInfo? journeyInfo;
  late final f.Either<api.OpaqueJourneyData, import_api.RawVectorData>
  journeyDataMaybeRaw;
  api.MapRendererProxy? _mapRendererProxy;
  MapBounds? _initialMapBounds;
  late import_api.ImportPreprocessor _preprocessor;
  List<import_api.VectorImportPartSummary> _vectorParts = const [];
  bool _isInitializing = true;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _initFlow();
    });
  }

  Future<void> _initFlow() async {
    try {
      await GlobalLoadingManager.instance.runWithWakelock(
        () => _loadFile(widget.path),
      );
      if (!mounted) return;
      var importSeparately = false;
      if (_vectorParts.length > 1) {
        importSeparately = await showCommonDialog(
          context,
          context.tr(
            'import.vector_multi.mode_message',
            args: ['${_vectorParts.length}'],
          ),
          title: context.tr('import.vector_multi.mode_title'),
          hasCancel: true,
          confirmButtonText: context.tr('import.vector_multi.separate_action'),
          cancelButtonText: context.tr('import.vector_multi.single_action'),
        );
        if (!mounted) return;
      }
      await showAutoSelectedPreprocessorNotice(
        context,
        preprocessor: _preprocessor,
      );
      if (!mounted) return;
      if (importSeparately) {
        final vectorData = switch (journeyDataMaybeRaw) {
          f.Right(value: final data) => data,
          f.Left() => throw StateError('Expected vector import data'),
        };
        await Navigator.of(context).pushReplacement(
          MaterialPageRoute(
            builder: (context) => GlobalPopScope(
              child: VectorMultiImportPage(
                vectorData: vectorData,
                parts: _vectorParts,
                initialPreprocessor: _preprocessor,
              ),
            ),
          ),
        );
        return;
      }
      final hasData = await GlobalLoadingManager.instance.runWithWakelock(
        _previewDataInternal,
      );
      if (!mounted) return;
      if (!hasData) {
        await showCommonDialog(context, context.tr("import.empty_data"));
        if (!mounted) return;
        popCurrentRoute(context);
        return;
      }
      setState(() => _isInitializing = false);
    } catch (error) {
      log.error("[import_data] Data parsing failed $error");
      if (!mounted) return;
      await showCommonDialog(context, context.tr("import.parsing_failed"));
      if (!mounted) return;
      popCurrentRoute(context);
    }
  }

  Future<void> _loadFile(String path) async {
    switch (widget.importType) {
      case ImportType.fow:
        var (journeyInfo, journeyData) = await import_api.loadFowData(
          filePath: path,
        );
        setState(() {
          this.journeyInfo = journeyInfo;
          journeyDataMaybeRaw = f.Either.left(journeyData);
          _preprocessor = import_api.ImportPreprocessor.generic;
        });
        break;

      case ImportType.vector:
        var (journeyInfo, rawVectorData, detectedProcessor) = await import_api
            .loadVectorData(filePath: path);
        final parts = await import_api.analyzeVectorDataByDate(
          vectorData: rawVectorData,
        );
        setState(() {
          this.journeyInfo = journeyInfo;
          _preprocessor = detectedProcessor;
          journeyDataMaybeRaw = f.Either.right(rawVectorData);
          _vectorParts = parts;
        });
        break;
    }
  }

  Future<void> _previewData(import_api.ImportPreprocessor preprocessor) async {
    if (preprocessor == _preprocessor) return;

    setState(() {
      _preprocessor = preprocessor;
    });

    if (!await showLoadingDialog(
      asyncTask: () async {
        return await _previewDataInternal();
      }(),
    )) {
      if (!mounted) return;
      await showCommonDialog(context, context.tr("import.empty_data"));
    }
  }

  Future<bool> _previewDataInternal() async {
    final journeyData = switch (journeyDataMaybeRaw) {
      f.Left(value: final l) => l,
      f.Right(value: final r) => await import_api.processVectorData(
        vectorData: r,
        importProcessor: _preprocessor,
      ),
    };
    final rendererAndBounds = await api.getMapRendererProxyForJourneyData(
      journeyData: journeyData,
    );

    setState(() {
      _mapRendererProxy = rendererAndBounds.$1;
      _initialMapBounds = rendererAndBounds.$2;
    });

    return !await import_api.isJourneyDataEmpty(journeyData: journeyData);
  }

  Future<void> _saveData(
    import_api.JourneyInfo journeyInfo,
    import_api.ImportPreprocessor processor,
  ) async {
    final success = await showLoadingDialog<bool>(
      asyncTask: (() async {
        final journeyDataMaybeRaw = this.journeyDataMaybeRaw;
        final journeyData = switch (journeyDataMaybeRaw) {
          f.Left(value: final l) => l,
          f.Right(value: final r) => await import_api.processVectorData(
            vectorData: r,
            importProcessor: processor,
          ),
        };
        if (await import_api.isJourneyDataEmpty(journeyData: journeyData)) {
          return false;
        }
        await import_api.importJourneyData(
          journeyInfo: journeyInfo,
          journeyData: journeyData,
        );
        return true;
      })(),
    );
    if (!mounted) return;
    if (success) {
      await showCommonDialog(context, context.tr("import.successful"));
    } else {
      await showCommonDialog(context, context.tr("import.empty_data"));
      // Blocking the return after the save process completes
      throw Exception("[import_data] Save data is empty");
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_isInitializing) {
      return ImportLoadingScaffold(filePath: widget.path);
    }

    final journeyInfo = this.journeyInfo;
    final panelHeight = widget.importType == ImportType.vector ? 530.0 : 510.0;
    final mapBoundsPadding =
        CapsuleStyleOverlayAppBar.mapFitPaddingForBottomOverlay(
          context,
          bottomOverlayHeight: panelHeight,
        );

    return Scaffold(
      body: journeyInfo == null
          ? const SizedBox.shrink()
          : Stack(
              children: [
                SlidingUpPanel(
                  color: StyleConstants.canvasColor,
                  borderRadius: const BorderRadius.only(
                    topLeft: Radius.circular(16.0),
                    topRight: Radius.circular(16.0),
                  ),
                  maxHeight: panelHeight,
                  defaultPanelState: PanelState.OPEN,
                  panel: PointerInterceptor(
                    child: Center(
                      child: Column(
                        children: [
                          Padding(
                            padding: const EdgeInsets.only(top: 12.0),
                            child: CustomPaint(
                              size: const Size(40.0, 4.0),
                              painter: LinePainter(
                                color: StyleConstants.mutedInkColor.withValues(
                                  alpha: 0.44,
                                ),
                              ),
                            ),
                          ),
                          const SizedBox(height: 16.0),
                          JourneyInfoEditPage(
                            startTime: journeyInfo.startTime,
                            endTime: journeyInfo.endTime,
                            journeyDate: journeyInfo.journeyDate.toSimpleDate(),
                            note: journeyInfo.note,
                            saveData: (journeyInfo, preprocessor) =>
                                _saveData(journeyInfo, preprocessor),
                            previewData: _previewData,
                            importType: widget.importType,
                            preprocessor: _preprocessor,
                          ),
                        ],
                      ),
                    ),
                  ),
                  body: _mapRendererProxy == null
                      ? const SizedBox.shrink()
                      : BaseMapWebview(
                          key: const ValueKey("mapWidget"),
                          mapRendererProxy: _mapRendererProxy!,
                          initialMapBounds: _initialMapBounds,
                          initialMapBoundsPadding: mapBoundsPadding,
                        ),
                ),
                CapsuleStyleOverlayAppBar.overlayBar(
                  title: context.tr("data.import_data.title"),
                ),
              ],
            ),
    );
  }
}
