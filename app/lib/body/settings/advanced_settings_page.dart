import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/map/gps_manager.dart';
import 'package:memolanes/body/settings/raw_data_page.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';

class AdvancedSettingsPage extends StatefulWidget {
  const AdvancedSettingsPage({super.key});

  @override
  State<AdvancedSettingsPage> createState() => _AdvancedSettingsPageState();
}

class _AdvancedSettingsPageState extends State<AdvancedSettingsPage> {
  @override
  Widget build(BuildContext context) {
    var gpsManager = context.watch<GpsManager>();

    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("general.advance_settings.title")),
      ),
      body: MlSingleChildScrollView(
        padding: EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("journey.delete_all"),
            position: LabelTilePosition.top,
            onTap: () async {
              if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
                await showCommonDialog(
                  context,
                  context.tr("journey.stop_ongoing_journey"),
                );
                return;
              }
              if (!await showCommonDialog(
                context,
                context.tr("journey.delete_all_journey_message"),
                hasCancel: true,
                title: context.tr("journey.delete_journey_title"),
                confirmButtonText: context.tr("common.delete"),
                confirmGroundColor: Colors.red,
                confirmTextColor: Colors.white,
              )) {
                return;
              }
              try {
                await api.deleteAllJourneys();
                if (context.mounted) {
                  await showCommonDialog(context, "All journeys are deleted.");
                }
              } catch (e) {
                if (context.mounted) {
                  await showCommonDialog(context, e.toString());
                }
              }
            },
          ),
          LabelTile(
            label: context.tr("db_optimization.button"),
            position: LabelTilePosition.middle,
            onTap: () async {
              if (!await api.mainDbRequireOptimization()) {
                if (!context.mounted) return;
                await showCommonDialog(
                  context,
                  context.tr("db_optimization.already_optimized"),
                );
              } else {
                if (!context.mounted) return;
                if (await showCommonDialog(
                  context,
                  context.tr("db_optimization.confirm"),
                  hasCancel: true,
                )) {
                  if (!context.mounted) return;
                  await showLoadingDialog(
                    context: context,
                    asyncTask: api.optimizeMainDb(),
                  );
                  if (!context.mounted) return;
                  await showCommonDialog(
                    context,
                    context.tr("db_optimization.finish"),
                  );
                }
              }
            },
          ),
          LabelTile(
            label: context.tr("general.advance_settings.export_logs"),
            position: LabelTilePosition.middle,
            onTap: () async {
              var tmpDir = await getTemporaryDirectory();
              var ts = DateTime.now().millisecondsSinceEpoch;
              var filepath = "${tmpDir.path}/${ts.toString()}.zip";
              await api.exportLogs(targetFilePath: filepath);
              if (!context.mounted) return;
              await showCommonExport(context, filepath, deleteFile: true);
            },
          ),
          LabelTile(
            label: context.tr("general.advance_settings.raw_data_mode"),
            position: LabelTilePosition.middle,
            onTap: () => Navigator.push(
              context,
              MaterialPageRoute(
                builder: (context) {
                  return RawDataPage();
                },
              ),
            ),
          ),
          LabelTile(
            label: context.tr("general.advance_settings.rebuild_cache"),
            position: LabelTilePosition.middle,
            onTap: () async => await showLoadingDialog(
              context: context,
              asyncTask: api.rebuildCache(),
            ),
          ),
          LabelTile(
            label: context.tr("location_service.location_backend.title"),
            position: LabelTilePosition.bottom,
            trailing: LabelTileContent(
                content: gpsManager.locationBackend.displayName(context)),
          )
        ],
      ),
    );
  }
}
