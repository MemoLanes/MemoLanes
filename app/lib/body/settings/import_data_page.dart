import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fpdart/fpdart.dart' as f;
import 'package:memolanes/body/journey/journey_edit_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/journey_data.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

class ImportDataPage extends StatefulWidget {
  const ImportDataPage(
      {super.key, required this.path, required this.importType});

  final String path;
  final ImportType importType;

  @override
  State<ImportDataPage> createState() => _ImportDataPage();
}

enum ImportType { fow, gpxOrKml }

class _ImportDataPage extends State<ImportDataPage> {
  import_api.JourneyInfo? journeyInfo;
  f.Either<JourneyData, import_api.RawVectorData>? journeyDataMaybeRaw;
  api.MapRendererProxy? _mapRendererProxy;
  MapView? _initialMapView;

  import_api.ImportPreprocessor _currentProcessor =
      import_api.ImportPreprocessor.generic;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _loadData();
    });
  }

  Future<void> _loadData() async {
    final future = () async {
      if (journeyDataMaybeRaw == null) {
        final result = await _readFile(widget.path, widget.importType);

        final (loadedJourneyInfo, loadedData) = result;

        if (loadedJourneyInfo == null || loadedData == null) {
          return false;
        }

        journeyInfo = loadedJourneyInfo;
        journeyDataMaybeRaw = loadedData;
      }

      try {
        final currentJourneyData = journeyDataMaybeRaw;
        if (currentJourneyData != null) {
          return await _previewDataInternal(
              currentJourneyData, _currentProcessor);
        }
        return false;
      } catch (e) {
        log.error("[import_data] Preview failed: $e");
        return false;
      }
    }();

    try {
      final success = await showLoadingDialog(
        context: context,
        asyncTask: future,
      );

      if (!mounted) return;

      if (!success) {
        await showCommonDialog(
          context,
          context.tr("import.empty_data"),
        );
        if (Navigator.canPop(context)) {
          Navigator.pop(context);
        }
      }
    } catch (error) {
      if (!mounted) return;

      await showCommonDialog(context, context.tr("import.parsing_failed"));
      log.error("[import_data] Data parsing failed $error");
      if (Navigator.canPop(context)) {
        Navigator.pop(context);
      }
    }
  }

  Future<
      (
        import_api.JourneyInfo?,
        f.Either<JourneyData, import_api.RawVectorData>?
      )> _readFile(String path, ImportType importType) async {
    switch (importType) {
      case ImportType.fow:
        var (journeyInfo, journeyData) =
            await import_api.loadFowData(filePath: path);
        return (
          journeyInfo,
          f.Either<JourneyData, import_api.RawVectorData>.left(journeyData)
        );

      case ImportType.gpxOrKml:
        var (journeyInfo, rawVectorData) =
            await import_api.loadGpxOrKml(filePath: path);
        return (
          journeyInfo,
          f.Either<JourneyData, import_api.RawVectorData>.right(rawVectorData)
        );
    }
  }

  Future<void> _previewData(import_api.ImportPreprocessor processor) async {
    if (processor == _currentProcessor) return;
    _currentProcessor = processor;
    _loadData();
  }

  Future<bool> _previewDataInternal(
      f.Either<JourneyData, import_api.RawVectorData> journeyDataMaybeRaw,
      import_api.ImportPreprocessor processor) async {
    final journeyData = switch (journeyDataMaybeRaw) {
      f.Left(value: final l) => l,
      f.Right(value: final r) => await import_api.processVectorData(
          vectorData: r, importProcessor: processor),
    };
    final mapRendererProxyAndCameraOption =
        await api.getMapRendererProxyForJourneyData(journeyData: journeyData);
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
    return true;
  }

  Future<void> _saveData(import_api.JourneyInfo journeyInfo,
      import_api.ImportPreprocessor processor) async {
    final success = await showLoadingDialog<bool>(
      context: context,
      asyncTask: (() async {
        final journeyDataMaybeRaw = this.journeyDataMaybeRaw;
        if (journeyDataMaybeRaw == null) {
          return false;
        }

        final journeyData = switch (journeyDataMaybeRaw) {
          f.Left(value: final l) => l,
          f.Right(value: final r) => await import_api.processVectorData(
              vectorData: r, importProcessor: processor),
        };
        await import_api.importJourneyData(
            journeyInfo: journeyInfo, journeyData: journeyData);
        return true;
      })(),
    );
    if (success) {
      await showCommonDialog(
        context,
        context.tr("import.successful"),
      );
    } else {
      await showCommonDialog(
        context,
        context.tr("import.empty_data"),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final journeyInfo = this.journeyInfo;

    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("data.import_data.title")),
      ),
      body: journeyInfo == null
          ? const SizedBox.shrink()
          : SlidingUpPanel(
              color: Colors.black,
              borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(16.0),
                topRight: Radius.circular(16.0),
              ),
              maxHeight: widget.importType == ImportType.gpxOrKml ? 530 : 510,
              defaultPanelState: PanelState.OPEN,
              panel: PointerInterceptor(
                child: SafeAreaWrapper(
                  child: Center(
                    child: Column(
                      children: [
                        Padding(
                          padding: const EdgeInsets.only(top: 12.0),
                          child: CustomPaint(
                            size: const Size(40.0, 4.0),
                            painter:
                                LinePainter(color: const Color(0xFFB5B5B5)),
                          ),
                        ),
                        const SizedBox(height: 16.0),
                        JourneyInfoEditPage(
                          startTime: journeyInfo.startTime,
                          endTime: journeyInfo.endTime,
                          journeyDate: journeyInfo.journeyDate,
                          note: journeyInfo.note,
                          saveData: _saveData,
                          previewData: _previewData,
                          importType: widget.importType,
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              body: _mapRendererProxy == null
                  ? const SizedBox.shrink()
                  : BaseMapWebview(
                      key: const ValueKey("mapWidget"),
                      mapRendererProxy: _mapRendererProxy!,
                      initialMapView: _initialMapView,
                    ),
            ),
    );
  }
}
