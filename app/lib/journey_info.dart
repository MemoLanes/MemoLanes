import 'dart:io';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/component/base_map.dart';
import 'package:path_provider/path_provider.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:share_plus/share_plus.dart';

class JourneyInfoPage extends StatefulWidget {
  const JourneyInfoPage({super.key, required this.journeyHeader});

  final JourneyHeader journeyHeader;

  @override
  State<JourneyInfoPage> createState() => _JourneyInfoPage();
}

class _JourneyInfoPage extends State<JourneyInfoPage> {
  final fmt = DateFormat('yyyy-MM-dd HH:mm:ss');
  api.MapRendererProxy? _mapRendererProxy;
  CameraOptions _cameraOptions = CameraOptions();

  @override
  void initState() {
    super.initState();
    api
        .getMapRendererProxyForJourney(journeyId: widget.journeyHeader.id)
        .then((mapRendererProxyAndCameraOption) {
      setState(() {
        _mapRendererProxy = mapRendererProxyAndCameraOption.$1;
        var cameraOption = mapRendererProxyAndCameraOption.$2;
        if (cameraOption != null) {
          _cameraOptions.zoom = cameraOption.zoom;
          _cameraOptions.center =
              Point(coordinates: Position(cameraOption.lng, cameraOption.lat));
        }
      });
    });
  }

  _export(JourneyHeader journeyHeader, api.ExportType exportType) async {
    var tmpDir = await getTemporaryDirectory();
    var filepath =
        "${tmpDir.path}/${journeyHeader.revision}.${exportType.name}";
    await api.exportJourney(
        targetFilepath: filepath,
        journeyId: journeyHeader.id,
        exportType: exportType);
    await Share.shareXFiles([XFile(filepath)]);
    try {
      await File(filepath).delete();
    } catch (e) {
      print(e);
      // don't care about error
    }
  }

  // TODO: Consider merge this one with the one in `utils.dart`
  showDialogFunction(fn) {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text("Delete"),
          content: const Text("Delete this record?"),
          actions: [
            TextButton(
              onPressed: () {
                Navigator.of(context).pop();
              },
              child: const Text('Cancel'),
            ),
            TextButton(onPressed: fn, child: const Text("Yes")),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = _mapRendererProxy;
    return Scaffold(
      appBar: AppBar(
        title: const Text("Journey Info"),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Text("Journey ID: ${widget.journeyHeader.id}"),
            Text(
                "Journey Date: ${naiveDateToString(date: widget.journeyHeader.journeyDate)}"),
            Text(
                "Start Time: ${widget.journeyHeader.start != null ? fmt.format(widget.journeyHeader.start!.toLocal()) : ""}"),
            Text(
                "End Time: ${widget.journeyHeader.end != null ? fmt.format(widget.journeyHeader.end!.toLocal()) : ""}"),
            Text(
                "Created At: ${fmt.format(widget.journeyHeader.createdAt.toLocal())}"),
            Text("Revision: ${widget.journeyHeader.revision}"),
            Text("Note: ${widget.journeyHeader.note}"),
            Row(mainAxisAlignment: MainAxisAlignment.spaceEvenly, children: [
              ElevatedButton(
                onPressed: widget.journeyHeader.journeyType ==
                        JourneyType.vector
                    ? () => _export(widget.journeyHeader, api.ExportType.kml)
                    : null,
                child: const Text("export KML"),
              ),
              ElevatedButton(
                onPressed: widget.journeyHeader.journeyType ==
                        JourneyType.vector
                    ? () => _export(widget.journeyHeader, api.ExportType.gpx)
                    : null,
                child: const Text("export GPX"),
              ),
              ElevatedButton(
                onPressed: () async {
                  showDialogFunction(() async {
                    Navigator.of(context).pop();
                    await api.deleteJourney(journeyId: widget.journeyHeader.id);
                    if (!context.mounted) return;
                    Navigator.pop(context, true);
                  });
                },
                child: const Text("delete"),
              ),
            ]),
            Expanded(
              child: mapRendererProxy == null
                  ? (const CircularProgressIndicator())
                  : (BaseMap(
                      key: const ValueKey("mapWidget"),
                      mapRendererProxy: mapRendererProxy,
                      initialCameraOptions: _cameraOptions,
                    )),
            )
          ],
        ),
      ),
    );
  }
}
