import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';

class AdvancedExportPage extends StatelessWidget {
  const AdvancedExportPage({super.key});

  @override
  Widget build(BuildContext context) {
    final gpsManager = context.watch<GpsManager>();

    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("data.export_data.advanced_export"),
      ),
      body: MlSingleChildScrollView(
        padding: EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("data.export_data.export_all_fwss"),
            position: LabelTilePosition.single,
            onTap: () async {
              if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
                await showCommonDialog(
                  context,
                  context.tr("journey.stop_ongoing_journey"),
                );
                return;
              }
              final tmpDir = await getTemporaryDirectory();
              final now = DateTime.now();
              final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss').format(now);
              final filepath = "${tmpDir.path}/all-journeys-$timestamp.fwss";
              if (!context.mounted) return;
              await showLoadingDialog(
                asyncTask: api.exportAllJourneysFwss(targetFilepath: filepath),
              );
              if (!context.mounted) return;
              await showCommonExport(context, filepath, deleteFile: true);
            },
          ),
        ],
      ),
    );
  }
}
