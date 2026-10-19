import 'package:memolanes/theme/app_colors.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:fpdart/fpdart.dart' as f;
import 'package:memolanes/body/journey/journey_info_form.dart';
import 'package:memolanes/body/journey/journey_info_fields.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/body/settings/import_preprocessor_notice.dart';
import 'package:memolanes/body/settings/vector_multi_import_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/back_button_target.dart';
import 'package:memolanes/common/component/import_loading_page.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/import_data.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class ImportDataPage extends StatefulWidget {
  const ImportDataPage({
    super.key,
    required this.path,
    required this.importType,
    this.onImportContinued,
  });

  final String path;
  final ImportType importType;

  /// Reports the final page's lifetime when this entry route is replaced.
  final ValueChanged<Future<dynamic>>? onImportContinued;

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

enum ImportType { fow, vector }

class _ImportDataPage extends State<ImportDataPage> {
  JourneyInfo? journeyInfo;
  late final f.Either<api.OpaqueJourneyData, import_api.RawVectorData>
  journeyDataMaybeRaw;
  api.MapRendererProxy? _mapRendererProxy;
  MapBounds? _initialMapBounds;
  final _detailPanelKey = GlobalKey<MapOverlayPanelState>();
  late ImportPreprocessor _preprocessor;
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
        final previewClosed = Navigator.of(context).pushReplacement(
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
        widget.onImportContinued?.call(previewClosed);
        await previewClosed;
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
          _preprocessor = ImportPreprocessor.generic;
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

  Future<void> _previewData(ImportPreprocessor preprocessor) async {
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
    JourneyInfo journeyInfo,
    ImportPreprocessor processor,
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
      if (mounted) popCurrentRoute(context, true);
    } else {
      await showCommonDialog(context, context.tr("import.empty_data"));
    }
  }

  Widget _buildInfoForm(JourneyInfo journeyInfo) => JourneyInfoForm(
    startTime: journeyInfo.startTime,
    endTime: journeyInfo.endTime,
    journeyDate: journeyInfo.journeyDate.toSimpleDate(),
    note: journeyInfo.note,
    journeyKind: journeyInfo.journeyKind,
    onSave: (journeyInfo) => _saveData(journeyInfo, _preprocessor),
    extraFields: widget.importType == ImportType.vector
        ? ImportPreprocessorTile(
            compact: true,
            value: _preprocessor,
            onSelected: _previewData,
            onInfoTap: () => showCommonDialog(
              context,
              context.tr('import.preprocessor.description_md'),
              markdown: true,
            ),
          )
        : null,
  );

  @override
  Widget build(BuildContext context) {
    if (_isInitializing) {
      return ImportLoadingScaffold(filePath: widget.path);
    }

    final journeyInfo = this.journeyInfo;
    final mapRendererProxy = _mapRendererProxy;
    final page = Scaffold(
      resizeToAvoidBottomInset: false,
      backgroundColor: context.appColors.canvasColor,
      body: journeyInfo == null
          ? const SizedBox.shrink()
          : AppLayoutBuilder(
              builder: (context, _) => Stack(
                fit: StackFit.expand,
                children: [
                  if (mapRendererProxy != null)
                    BaseMapWebview(
                      key: const ValueKey('mapWidget'),
                      mapRendererProxy: mapRendererProxy,
                      initialMapBounds: _initialMapBounds,
                      overlayPanel: _detailPanelKey,
                    ),
                  MapOverlayPanel.detail(
                    key: _detailPanelKey,
                    child: PointerInterceptor(
                      child: JourneyInfoPanelSurface(
                        child: SingleChildScrollView(
                          padding: const EdgeInsets.only(top: 10),
                          child: _buildInfoForm(journeyInfo),
                        ),
                      ),
                    ),
                  ),
                  BackButtonTarget.positioned(
                    onPressed: () => Navigator.maybePop(context),
                    child: MapGlassBackButton(
                      onPressed: () => Navigator.maybePop(context),
                    ),
                  ),
                ],
              ),
            ),
    );

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: AppTheme.mapSystemOverlayStyle(Theme.of(context)),
      child: page,
    );
  }
}
