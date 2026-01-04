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

  @override
  void initState() {
    super.initState();
    _loadMap();
  }

  Future<void> _loadMap() async {
    final result =
        await api.getMapRendererProxyForJourney(journeyId: widget.journeyId);
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
                mapRendererProxy: _mapRendererProxy!,
                initialMapView: _initialMapView,
                trackingMode: TrackingMode.off,
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
                    backgroundColor: Colors.red,
                    foregroundColor: Colors.white,
                    onPressed: () {
                      // TODO: Implement delete track functionality
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                            content: Text("Delete track not implemented yet")),
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
                    onPressed: () {
                      // TODO: Implement save track functionality
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                            content: Text("Save track not implemented yet")),
                      );
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
