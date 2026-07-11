import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/body/settings/raw_data_page.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:memolanes/body/settings/render_diagnostics.dart';

class AdvancedSettingsPage extends StatefulWidget {
  const AdvancedSettingsPage({super.key});

  @override
  State<AdvancedSettingsPage> createState() => _AdvancedSettingsPageState();
}

class _AdvancedSettingsPageState extends State<AdvancedSettingsPage> {
  late Worldview _worldview;

  @override
  void initState() {
    super.initState();
    _worldview = WorldviewManager.instance.currentWorldview;
  }

  Future<void> _selectWorldview() async {
    final result = await showWorldviewPicker(
      context,
      selectedWorldview: _worldview,
    );
    if (result == null || result == _worldview || !mounted) return;

    await showLoadingDialog(
      asyncTask: WorldviewManager.instance.update(result),
    );
    if (!mounted) return;
    setState(() => _worldview = result);
  }

  @override
  Widget build(BuildContext context) {
    var gpsManager = context.watch<GpsManager>();

    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("general.advanced_settings.title"),
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
                  await showCommonDialog(
                      context, context.tr("journey.delete_all_success"));
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
            label: context.tr("general.advanced_settings.export_logs"),
            position: LabelTilePosition.middle,
            onTap: () async {
              var tmpDir = await getTemporaryDirectory();
              final now = DateTime.now();
              final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss').format(now);
              final filepath = "${tmpDir.path}/logs-$timestamp.zip";
              await api.exportLogs(targetFilePath: filepath);
              if (!context.mounted) return;
              await showCommonExport(
                context,
                filepath,
                deleteFile: true,
              );
            },
          ),
          LabelTile(
            label: context.tr("general.advanced_settings.raw_data_mode"),
            position: LabelTilePosition.middle,
            onTap: () => navigatorPush(context, page: RawDataPage()),
          ),
          LabelTile(
            label: context.tr("general.advanced_settings.rebuild_cache"),
            position: LabelTilePosition.middle,
            onTap: () async => await showLoadingDialog(
              asyncTask: api.rebuildCache(),
            ),
          ),
          LabelTile(
            label: context.tr("privacy.region_title"),
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(
              content: regionPreferenceTitle(context, _worldview),
              showArrow: true,
            ),
            onTap: _selectWorldview,
          ),
          LabelTile(
            label: context.tr("general.advanced_settings.reset_local_prefs"),
            position: LabelTilePosition.middle,
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
                context
                    .tr("general.advanced_settings.reset_local_prefs_message"),
                hasCancel: true,
                title:
                    context.tr("general.advanced_settings.reset_local_prefs"),
                confirmButtonText: context.tr("common.reset"),
                confirmGroundColor: Colors.red,
                confirmTextColor: Colors.white,
              )) {
                return;
              }
              MMKVUtil.clearAll();
              exit(0);
            },
          ),
          LabelTile(
            label: context.tr("location_service.location_backend.title"),
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(
                content: gpsManager.locationBackend.displayName(context)),
          ),
          LabelTile(
            label: context.tr("haptics.setting_title"),
            position: LabelTilePosition.middle,
            trailing: Switch(
              value: AppHaptics.isUserHapticsEnabled,
              onChanged: (value) {
                AppHaptics.setUserHapticsEnabled(value);
                // Confirm when turning on.
                if (value) AppHaptics.selection();
                setState(() {});
              },
            ),
          ),
          LabelTile(
            label: context.tr("general.advanced_settings.render_diagnostics"),
            position: LabelTilePosition.bottom,
            onTap: () => navigatorPush(context, page: RenderDiagnosticsPage()),
          )
        ],
      ),
    );
  }
}
