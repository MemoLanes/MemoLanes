import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/settings/debug/debug_settings_page.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/body/settings/location_service_settings.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/theme/app_colors.dart';
import 'package:memolanes/utils/nav_helper.dart';
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
    final gpsManager = context.watch<GpsManager>();
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('settings.categories.advanced.title'),
      ),
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.diagnostics'),
            children: [
              SettingsTile(
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
              locationBackendSettingsTile(context, gpsManager),
              if (gpsManager.locationBackend.supportsSmartPowerSaving)
                SettingsTile(
                  label: context.tr(
                    'general.advanced_settings.smart_power_saving',
                  ),
                  position: LabelTilePosition.middle,
                  trailing: Switch(
                    value: MMKVUtil.getBool(
                      MMKVKey.smartPowerSavingEnabled,
                      defaultValue: true,
                    ),
                    onChanged: (value) =>
                        _setSmartPowerSaving(value, gpsManager),
                  ),
                ),
              SettingsTile(
                label: context.tr('general.advanced_settings.debug.title'),
                position: LabelTilePosition.bottom,
                bottom: false,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () =>
                    navigatorPush(context, page: const DebugSettingsPage()),
              ),
            ],
          ),
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.recovery'),
            children: [
              SettingsTile(
                label: context.tr(
                  'general.advanced_settings.reset_local_prefs',
                ),
                position: LabelTilePosition.single,
                bottom: false,
                onTap: () => _resetPreferences(context, gpsManager),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _setSmartPowerSaving(bool enabled, GpsManager gpsManager) async {
    if (!MMKVUtil.putBool(MMKVKey.smartPowerSavingEnabled, enabled)) {
      log.error(
        '[Settings] Failed to save smart power saving (enabled=$enabled)',
        StackTrace.current,
      );
      return;
    }
    setState(() {});
    if (gpsManager.recordingStatus == GpsRecordingStatus.recording) {
      await showCommonDialog(
        context,
        context.tr('journey.setting_applies_next_recording'),
      );
    }
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
