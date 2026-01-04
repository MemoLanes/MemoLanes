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
  bool _isDeleteMode = false;
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
      });
    }
  }

  void _onTrackSelected(double lat, double lng) async {
    if (_isDeleteMode) {
      final result = await api.deletePointInEdit(lat: lat, lng: lng);
      if (mounted) {
        setState(() {
          _mapRendererProxy = result.$1;
        });
      }
    }
  }

  void _onSelectionBox(
      double startLat, double startLng, double endLat, double endLng) async {
    if (_isDeleteMode) {
      final result = await api.deletePointsInBoxInEdit(
          startLat: startLat,
          startLng: startLng,
          endLat: endLat,
          endLng: endLng);
      if (mounted) {
        setState(() {
          _mapRendererProxy = result.$1;
        });
      }
    }
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
                onTrackSelected: _onTrackSelected,
                onSelectionBox: _onSelectionBox,
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
                  FloatingActionButton(
                    heroTag: "delete_track",
                    backgroundColor: _isDeleteMode ? Colors.red : Colors.grey,
                    foregroundColor: Colors.white,
                    onPressed: () {
                      setState(() {
                        _isDeleteMode = !_isDeleteMode;
                      });
                      _mapWebviewKey.currentState?.setDeleteMode(_isDeleteMode);
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                            content: Text(_isDeleteMode
                                ? "Delete mode enabled. Drag to select area to delete."
                                : "Delete mode disabled.")),
                      );
                    },
                    child: const Icon(Icons.cleaning_services),
                  ),
                  const SizedBox(width: 32),
                  FloatingActionButton(
                    heroTag: "add_track",
                    backgroundColor: Colors.green,
                    foregroundColor: Colors.white,
                    onPressed: () {
                      // TODO: Implement add track functionality
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                            content: Text("Add track not implemented yet")),
                      );
                    },
                    child: const Icon(Icons.edit),
                  ),
                  const SizedBox(width: 32),
                  FloatingActionButton(
                    heroTag: "save_track",
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
