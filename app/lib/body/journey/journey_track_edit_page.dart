import 'package:flutter/material.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:easy_localization/easy_localization.dart';

class JourneyTrackEditPage extends StatefulWidget {
  final String journeyId;

  const JourneyTrackEditPage({super.key, required this.journeyId});

  @override
  State<JourneyTrackEditPage> createState() => _JourneyTrackEditPageState();
}

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  api.MapRendererProxy? _mapRendererProxy;
  MapView? _initialMapView;
  bool _isAddMode = false;
  bool _isDeleteMode = false;
  bool _canUndo = false;
  final GlobalKey<BaseMapWebviewState> _mapWebviewKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    _loadMap();
  }

  Future<void> _loadMap() async {
    final result = await api.startJourneyEdit(journeyId: widget.journeyId);
    if (mounted) {
      setState(() {
        _mapRendererProxy = result.$1;
        final cameraOption = result.$2;
        if (cameraOption != null) {
          _initialMapView = (
            lng: cameraOption.lng,
            lat: cameraOption.lat,
            zoom: cameraOption.zoom,
          );
        }
        _canUndo = false;
      });
    }
  }

  Future<void> _refreshCanUndo() async {
    final canUndo = await api.canUndoInEdit();
    if (!mounted) return;
    setState(() {
      _canUndo = canUndo;
    });
  }

  Future<void> _onDrawPath(List<DrawPoint> points) async {
    if (!_isAddMode) return;
    if (points.length < 2) return;

    await api.pushUndoCheckpointInEdit();

    // Approximate the freehand path by adding many small straight segments.
    api.MapRendererProxy? latestProxy = _mapRendererProxy;
    for (var i = 0; i < points.length - 1; i++) {
      final a = points[i];
      final b = points[i + 1];
      final result = await api.addLineInEdit(
        startLat: a.lat,
        startLng: a.lng,
        endLat: b.lat,
        endLng: b.lng,
      );
      latestProxy = result.$1;
    }

    if (!mounted) return;
    if (latestProxy != null) {
      setState(() {
        _mapRendererProxy = latestProxy;
      });
    }

    await _refreshCanUndo();
  }

  Future<void> _onSelectionBox(
    double startLat,
    double startLng,
    double endLat,
    double endLng,
  ) async {
    if (!_isDeleteMode) return;

    await api.pushUndoCheckpointInEdit();

    final result = await api.deletePointsInBoxInEdit(
      startLat: startLat,
      startLng: startLng,
      endLat: endLat,
      endLng: endLng,
    );

    if (!mounted) return;
    setState(() {
      _mapRendererProxy = result.$1;
    });

    await _refreshCanUndo();
  }

  @override
  void dispose() {
    api.discardJourneyEdit();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("journey.journey_track_edit_title")),
      ),
      body: SafeAreaWrapper(
        child: Stack(
          children: [
            if (_mapRendererProxy != null)
              BaseMapWebview(
                key: _mapWebviewKey,
                mapRendererProxy: _mapRendererProxy!,
                initialMapView: _initialMapView,
                trackingMode: TrackingMode.off,
                onSelectionBox: _onSelectionBox,
                onDrawPath: _onDrawPath,
              )
            else
              const Center(child: CircularProgressIndicator()),
            Positioned(
              bottom: 16,
              left: 0,
              right: 0,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  if (_canUndo)
                    FloatingActionButton(
                      heroTag: "undo_track_edit",
                      backgroundColor: const Color(0xFFFFFFFF),
                      foregroundColor: Colors.black,
                      onPressed: () async {
                        final result = await api.undoInEdit();
                        if (!mounted) return;
                        setState(() {
                          _mapRendererProxy = result.$1;
                        });
                        await _refreshCanUndo();
                      },
                      child: const Icon(Icons.undo),
                    )
                  else
                    const SizedBox(width: 56, height: 56),
                  const SizedBox(width: 32),
                  FloatingActionButton(
                    heroTag: "add_track",
                    backgroundColor:
                        _isAddMode ? const Color(0xFFB6E13D) : Colors.grey,
                    foregroundColor: Colors.white,
                    onPressed: () {
                      setState(() {
                        _isAddMode = !_isAddMode;
                        if (_isAddMode) {
                          _isDeleteMode = false;
                        }
                      });
                      _mapWebviewKey.currentState?.setDeleteMode(false);
                      _mapWebviewKey.currentState?.setDrawMode(_isAddMode);
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                            content: Text(_isAddMode
                                ? context.tr(
                                    "journey.journey_track_edit_add_mode_enabled",
                                  )
                                : context.tr(
                                    "journey.journey_track_edit_add_mode_disabled",
                                  ))),
                      );
                    },
                    child: const Icon(Icons.edit),
                  ),
                  const SizedBox(width: 32),
                  FloatingActionButton(
                    heroTag: "delete_track",
                    backgroundColor:
                        _isDeleteMode ? const Color(0xFFE13D3D) : Colors.grey,
                    foregroundColor: Colors.white,
                    onPressed: () {
                      setState(() {
                        _isDeleteMode = !_isDeleteMode;
                        if (_isDeleteMode) {
                          _isAddMode = false;
                        }
                      });
                      _mapWebviewKey.currentState?.setDrawMode(false);
                      _mapWebviewKey.currentState?.setDeleteMode(_isDeleteMode);
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text(
                            _isDeleteMode
                                ? context.tr(
                                    "journey.journey_track_edit_delete_mode_enabled",
                                  )
                                : context.tr(
                                    "journey.journey_track_edit_delete_mode_disabled",
                                  ),
                          ),
                        ),
                      );
                    },
                    child: const Icon(Icons.delete),
                  ),
                  const SizedBox(width: 32),
                  FloatingActionButton(
                    heroTag: "save_track",
                    backgroundColor: const Color(0xFFFFFFFF),
                    onPressed: () async {
                      await api.saveJourneyEdit();
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                              content: Text(context.tr("common.save_success"))),
                        );
                        Navigator.pop(context);
                      }
                    },
                    child: const Icon(Icons.save),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
