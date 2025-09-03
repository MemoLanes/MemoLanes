import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:fpdart/fpdart.dart' as f;
import 'package:memolanes/body/journey/journey_edit_page.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/journey_data.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:sliding_up_panel/sliding_up_panel.dart';

import '../../common/utils.dart';

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

  @override
  void initState() {
    super.initState();
    _readData(widget.path);
  }

  void _readData(String path) async {
    try {
      switch (widget.importType) {
        case ImportType.fow:
          var (journeyInfo, journeyData) =
              await import_api.loadFowData(filePath: path);
          setState(() {
            this.journeyInfo = journeyInfo;
            journeyDataMaybeRaw = f.Either.left(journeyData);
          });
          break;
        case ImportType.gpxOrKml:
          var (journeyInfo, rawVectorData) =
              await import_api.loadGpxOrKml(filePath: path);
          setState(() {
            this.journeyInfo = journeyInfo;
            journeyDataMaybeRaw = f.Either.right(rawVectorData);
          });
          break;
      }
    } catch (error) {
      Fluttertoast.showToast(msg: "Data parsing failed");
      log.error("[import_data] Data parsing failed $error");
      Navigator.pop(context);
    }
  }

  void _previewData(import_api.ImportProcessor processor) async {
    final journeyDataMaybeRaw = this.journeyDataMaybeRaw;
    if (journeyDataMaybeRaw == null) {
      Fluttertoast.showToast(msg: "JourneyData is empty");
      return;
    }

    final journeyData = switch (journeyDataMaybeRaw) {
      f.Left(value: final l) => l,
      f.Right(value: final r) => await import_api.processVectorData(
          vectorData: r, importProcessor: processor),
    };
    if (journeyData == null) {
      await showCommonDialog(
        context,
        context.tr("No data found"),
      );
      return;
    }
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
  }

  void _saveData(import_api.JourneyInfo journeyInfo,
      import_api.ImportProcessor processor) async {
    final journeyDataMaybeRaw = this.journeyDataMaybeRaw;
    if (journeyDataMaybeRaw == null) {
      Fluttertoast.showToast(msg: "JourneyData is empty");
      return;
    }

    final journeyData = switch (journeyDataMaybeRaw) {
      f.Left(value: final l) => l,
      f.Right(value: final r) => await import_api.processVectorData(
          vectorData: r, importProcessor: processor),
    };
    if (journeyData == null) {
      await showCommonDialog(
        context,
        context.tr("No data found"),
      );
      return;
    }
    await import_api.importJourneyData(
        journeyInfo: journeyInfo, journeyData: journeyData);

    Fluttertoast.showToast(msg: "Import successful");
  }

  @override
  Widget build(BuildContext context) {
    var journeyInfo = this.journeyInfo;
    final mapRendererProxy = _mapRendererProxy;
    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("data.import_data.title")),
      ),
      body: journeyInfo == null
          ? Center(
              child: const Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    "Reading data, please wait",
                    style: TextStyle(fontSize: 22.0),
                  ),
                  CircularProgressIndicator()
                ],
              ),
            )
          : SlidingUpPanel(
              color: Colors.black,
              borderRadius: BorderRadius.only(
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
