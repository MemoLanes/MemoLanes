import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/render_diagnostics.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';

class AdvancedSettingsPage extends StatelessWidget {
  const AdvancedSettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final gpsManager = context.watch<GpsManager>();
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.advanced.title'),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8),
        children: [
          SettingsSection(
            title: context.tr('settings.groups.diagnostics'),
            children: [
              LabelTile(
                label: context.tr('general.advanced_settings.export_logs'),
                position: LabelTilePosition.top,
                onTap: () async {
                  final tmpDir = await getTemporaryDirectory();
                  final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss')
                      .format(DateTime.now());
                  final filepath = '${tmpDir.path}/logs-$timestamp.zip';
                  await api.exportLogs(targetFilePath: filepath);
                  if (context.mounted) {
                    await showCommonExport(context, filepath, deleteFile: true);
                  }
                },
              ),
              LabelTile(
                label: context.tr('location_service.location_backend.title'),
                position: LabelTilePosition.middle,
                trailing: LabelTileContent(
                  content: gpsManager.locationBackend.displayName(context),
                ),
              ),
              LabelTile(
                label: context.tr(
                  'general.advanced_settings.render_diagnostics',
                ),
                position: LabelTilePosition.bottom,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () =>
                    navigatorPush(context, page: const RenderDiagnosticsPage()),
              ),
            ],
          ),
          SettingsSection(
            title: context.tr('settings.groups.recovery'),
            children: [
              LabelTile(
                label: context.tr(
                  'general.advanced_settings.reset_local_prefs',
                ),
                position: LabelTilePosition.single,
                onTap: () => _resetPreferences(context, gpsManager),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _resetPreferences(
    BuildContext context,
    GpsManager gpsManager,
  ) async {
    if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
      await showCommonDialog(
        context,
        context.tr('journey.stop_ongoing_journey'),
      );
      return;
    }
    if (!await showCommonDialog(
      context,
      context.tr('general.advanced_settings.reset_local_prefs_message'),
      hasCancel: true,
      title: context.tr('general.advanced_settings.reset_local_prefs'),
      confirmButtonText: context.tr('common.reset'),
      confirmVariant: AppButtonVariant.danger,
    )) {
      return;
    }
    MMKVUtil.clearAll();
    exit(0);
  }
}
