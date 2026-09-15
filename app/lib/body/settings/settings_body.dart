import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/body/settings/advanced_settings_page.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/body/settings/interface_settings_page.dart';
import 'package:memolanes/body/settings/map_settings_page.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/component/tiles/label_tile_title.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording_health_service.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils/nav_helper.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:url_launcher/url_launcher_string.dart';

import 'contact_us_page.dart';

class SettingsBody extends StatefulWidget {
  const SettingsBody({super.key});

  @override
  State<SettingsBody> createState() => _SettingsBodyState();
}

class _SettingsBodyState extends State<SettingsBody> {
  bool _isUnexpectedExitNotificationEnabled = false;
  String _version = "";

  @override
  void initState() {
    super.initState();
    _loadNotificationStatus();
    _loadVersion();
  }

  Future<void> _loadVersion() async {
    final packageInfo = await PackageInfo.fromPlatform();
    if (!mounted) return;
    setState(() {
      _version =
          '${packageInfo.version} (${packageInfo.buildNumber}) [${api.shortCommitHash()}]';
    });
  }

  void _launchUrl(String updateUrl) async {
    final url = Uri.parse(updateUrl);
    if (await canLaunchUrl(url)) {
      await launchUrl(url, mode: LaunchMode.externalApplication);
    } else {
      throw 'Could not launch $updateUrl';
    }
  }

  Future<void> _selectImportFile(
    BuildContext context,
    ImportType importType,
  ) async {
    // TODO: FilePicker is weird and `allowedExtensions` does not really work.
    // https://github.com/miguelpruivo/flutter_file_picker/wiki/FAQ
    // List<String> allowedExtensions;
    // if (importType == ImportType.fow) {
    //   allowedExtensions = ['zip'];
    // } else {
    //   allowedExtensions = ['kml', 'gpx', 'csv'];
    // }
    final result = await FilePicker.pickFile(type: FileType.any);
    final path = result?.path;
    if (path != null && context.mounted) {
      navigatorPush(
        context,
        page: ImportDataPage(path: path, importType: importType),
      );
    }
  }

  void _loadNotificationStatus() {
    _isUnexpectedExitNotificationEnabled = MMKVUtil.getBool(
      MMKVKey.isUnexpectedExitNotificationEnabled,
      defaultValue: true,
    );
  }

  @override
  Widget build(BuildContext context) {
    // Settings cards read the shared palette directly, so explicitly rebuild
    // this route when a nested settings page changes that palette.
    context.watch<AppThemeController>();
    var updateUrl = context.watch<UpdateNotifier>().updateUrl;
    var gpsManager = context.watch<GpsManager>();

    return MlSingleChildScrollView(
      padding: EdgeInsets.only(
        top: 16.0,
        bottom: StyleConstants.navBarSafeArea + 16.0,
      ),
      children: [
        _SettingsPageHeader(),
        const SizedBox(height: 18),
        // TODO: Enable this when we have user system.
        // CircleAvatar(
        //   backgroundColor: StyleConstants.primaryGreen,
        //   radius: 45.0,
        // ),
        // Padding(
        //   padding: EdgeInsets.symmetric(vertical: 16.0),
        //   child: Text(
        //     'Foo Bar',
        //     style: TextStyle(
        //       fontSize: 24.0,
        //       color: StyleConstants.surfaceColor,
        //     ),
        //   ),
        // ),
        OptionCard(
          useSafeArea: false,
          separators: false,
          children: [
            LabelTileTitle(label: context.tr("general.title")),
            LabelTile(
              label: context.tr("general.version.title"),
              position: LabelTilePosition.middle,
              prefix: _SettingsTileIcon(icon: Icons.info_outline_rounded),
              trailing: updateUrl != null
                  ? badges.Badge(
                      badgeStyle: badges.BadgeStyle(
                        shape: badges.BadgeShape.square,
                        borderRadius: BorderRadius.circular(5),
                        padding: const EdgeInsets.all(2),
                        badgeColor: StyleConstants.deepGreen,
                      ),
                      badgeContent: Text(
                        'NEW',
                        style: AppTypography.badge.copyWith(
                          color: StyleConstants.inverseInkColor,
                        ),
                      ),
                      child: LabelTileContent(content: _version),
                    )
                  : LabelTileContent(content: _version),
              onTap: () async {
                if (updateUrl != null) {
                  _launchUrl(updateUrl);
                  return;
                }
                await showCommonDialog(
                  context,
                  context.tr("general.version.currently_the_latest_version"),
                );
              },
            ),
            LabelTile(
              label: context.tr("general.map_settings.title"),
              position: LabelTilePosition.middle,
              prefix: _SettingsTileIcon(icon: Icons.map_outlined),
              trailing: LabelTileContent(showArrow: true),
              onTap: () =>
                  navigatorPush(context, page: const MapSettingsPage()),
            ),
            LabelTile(
              label: context.tr("general.interface_settings.title"),
              position: LabelTilePosition.middle,
              prefix: _SettingsTileIcon(icon: Icons.palette_outlined),
              trailing: LabelTileContent(showArrow: true),
              onTap: () =>
                  navigatorPush(context, page: const InterfaceSettingsPage()),
            ),
            LabelTile(
              label: context.tr("general.advanced_settings.title"),
              position: LabelTilePosition.bottom,
              bottom: false,
              prefix: _SettingsTileIcon(icon: Icons.tune_rounded),
              trailing: LabelTileContent(showArrow: true),
              onTap: () => navigatorPush(context, page: AdvancedSettingsPage()),
            ),
          ],
        ),
        const SizedBox(height: 14),
        OptionCard(
          useSafeArea: false,
          separators: false,
          children: [
            LabelTileTitle(label: context.tr("data.title")),
            // TODO: This is unused, but we may use it depending on the design of
            // import/export workflow.
            //
            // LabelTile(
            //   label: context.tr("data.backup_data.title"),
            //   position: LabelTilePosition.middle,
            //   trailing: LabelTileContent(showArrow: true),
            //   onTap: () => Navigator.push(
            //     context,
            //     MaterialPageRoute(
            //       builder: (context) {
            //         return BackupDataScreen();
            //       },
            //     ),
            //   ),
            // ),
            LabelTile(
              label: context.tr("data.import_data.title"),
              position: LabelTilePosition.middle,
              prefix: _SettingsTileIcon(
                icon: Icons.file_download_outlined,
                yellow: true,
              ),
              trailing: LabelTileContent(showArrow: true),
              onTap: () => _showImportDataCard(context),
            ),
            LabelTile(
              label: context.tr("data.export_data.export_all"),
              position: LabelTilePosition.bottom,
              bottom: false,
              prefix: _SettingsTileIcon(
                icon: Icons.file_upload_outlined,
                yellow: true,
              ),
              onTap: () async {
                if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
                  await showCommonDialog(
                    context,
                    context.tr("journey.stop_ongoing_journey"),
                  );
                  return;
                }
                final hasJourneys = await api.hasJourneys();
                if (!context.mounted) return;
                if (!hasJourneys) {
                  await showCommonDialog(
                    context,
                    context.tr("data.export_data.error.no_journeys_to_export"),
                  );
                  return;
                }
                await showCommonExportWithFormatPicker(
                  context: context,
                  title: context.tr("data.export_data.export_all_title"),
                  formats: const [
                    CommonExportFormat.mldx,
                    CommonExportFormat.fwss,
                  ],
                  exportFile: (format) async {
                    var tmpDir = await getTemporaryDirectory();
                    final now = DateTime.now();
                    final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss')
                        .format(now);
                    final filepath =
                        "${tmpDir.path}/all-journeys-$timestamp.${format.extension}";
                    final exportResult = switch (format) {
                      CommonExportFormat.mldx => await api.generateFullArchive(
                        targetFilepath: filepath,
                      ),
                      CommonExportFormat.fwss =>
                        await api.exportAllJourneysAsFwss(
                          targetFilepath: filepath,
                        ),
                      CommonExportFormat.kml ||
                      CommonExportFormat.gpx => throw UnsupportedError(
                        'Unsupported export format: $format',
                      ),
                    };
                    return CommonExportResult.create(exportResult, filepath);
                  },
                );
              },
            ),
          ],
        ),
        const SizedBox(height: 14),
        OptionCard(
          useSafeArea: false,
          separators: false,
          children: [
            LabelTileTitle(label: context.tr("settings.other")),
            LabelTile(
              label: context.tr("unexpected_exit_notification.setting_title"),
              position: defaultTargetPlatform == TargetPlatform.android
                  ? LabelTilePosition.middle
                  : LabelTilePosition.bottom,
              bottom: false,
              prefix: _SettingsTileIcon(
                icon: Icons.notifications_active_outlined,
              ),
              trailing: Switch(
                value: _isUnexpectedExitNotificationEnabled,
                onChanged: (value) async {
                  final status = await Permission.notification.status;
                  if (!context.mounted) return;
                  if (value) {
                    if (!status.isGranted) {
                      setState(() {
                        _isUnexpectedExitNotificationEnabled = false;
                      });

                      await showCommonDialog(
                        context,
                        context.tr(
                          "unexpected_exit_notification.notification_permission_denied",
                        ),
                      );
                      Geolocator.openAppSettings();
                      return;
                    }
                  }
                  MMKVUtil.putBool(
                    MMKVKey.isUnexpectedExitNotificationEnabled,
                    value,
                  );
                  setState(() {
                    _isUnexpectedExitNotificationEnabled = value;
                  });
                  if (gpsManager.recordingStatus ==
                      GpsRecordingStatus.recording) {
                    if (!context.mounted) return;
                    await showCommonDialog(
                      context,
                      context.tr(
                        "unexpected_exit_notification.change_affect_next_time",
                      ),
                    );
                  }
                },
              ),
            ),
            if (defaultTargetPlatform == TargetPlatform.android)
              LabelTile(
                label: context.tr("recording_health.setting_title"),
                position: LabelTilePosition.bottom,
                bottom: false,
                prefix: _SettingsTileIcon(icon: Icons.monitor_heart_outlined),
                trailing: ListenableBuilder(
                  listenable: RecordingHealthService.instance,
                  builder: (context, _) => Switch(
                    value: RecordingHealthService
                        .instance
                        .isHeartbeatDetectionEnabled,
                    onChanged: (value) {
                      RecordingHealthService.instance
                          .setHeartbeatDetectionEnabled(
                            value,
                            recordingStatus: gpsManager.recordingStatus,
                          );
                    },
                  ),
                ),
              ),
          ],
        ),
        const SizedBox(height: 14),
        OptionCard(
          useSafeArea: false,
          separators: false,
          children: [
            LabelTileTitle(label: context.tr("settings.about")),
            LabelTile(
              label: context.tr("privacy.name"),
              position: LabelTilePosition.middle,
              prefix: _SettingsTileIcon(
                icon: Icons.shield_outlined,
                yellow: true,
              ),
              trailing: LabelTileContent(rightIcon: Icons.open_in_new),
              onTap: () async {
                await launchUrlString(
                  context.tr("privacy.url"),
                  mode: LaunchMode.externalApplication,
                );
              },
            ),
            LabelTile(
              label: context.tr("contact_us.title"),
              position: LabelTilePosition.bottom,
              bottom: false,
              prefix: _SettingsTileIcon(
                icon: Icons.forum_outlined,
                yellow: true,
              ),
              trailing: LabelTileContent(rightIcon: Icons.arrow_forward_ios),
              onTap: () => navigatorPush(context, page: ContactUsPage()),
            ),
          ],
        ),
      ],
    );
  }

  void _showImportDataCard(BuildContext context) {
    showBasicDialogCard<void>(
      context,
      title: context.tr("data.import_data.title"),
      maxHeightFactor: 0.68,
      contentPadding: const EdgeInsets.fromLTRB(16, 12, 16, 18),
      builder: (dialogContext) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            context.tr("data.import_data.description"),
            style: AppTypography.supporting.copyWith(
              color: StyleConstants.mutedInkColor,
            ),
          ),
          const SizedBox(height: 12),
          Column(
            children: [
              AppOptionTile(
                icon: Icons.inventory_2_outlined,
                title: context.tr("journey.import_mldx_data"),
                onTap: () async {
                  Navigator.of(dialogContext).pop();
                  // TODO: FilePicker is weird and `allowedExtensions` does not really work.
                  // https://github.com/miguelpruivo/flutter_file_picker/wiki/FAQ
                  var result = await FilePicker.pickFile(type: FileType.any);
                  if (!context.mounted) return;
                  if (result != null) {
                    var path = result.path;
                    if (path != null) {
                      await importMldx(context, path);
                    }
                  }
                },
              ),
              const SizedBox(height: 8),
              AppOptionTile(
                icon: Icons.route_outlined,
                title: context.tr("import.vector.title"),
                onTap: () async {
                  Navigator.of(dialogContext).pop();
                  await showCommonDialog(
                    context,
                    context.tr("import.vector.description_md"),
                    markdown: true,
                  );
                  if (!context.mounted) return;
                  await _selectImportFile(context, ImportType.vector);
                },
              ),
              const SizedBox(height: 8),
              AppOptionTile(
                icon: Icons.grid_4x4_outlined,
                title: context.tr("journey.import_fog_of_world_data"),
                onTap: () async {
                  Navigator.of(dialogContext).pop();
                  await showCommonDialog(
                    context,
                    context.tr("import.import_fow_data.description_md"),
                    markdown: true,
                  );
                  if (await api.containsBitmapJourney()) {
                    if (!context.mounted) return;
                    await showCommonDialog(
                      context,
                      context.tr(
                        "import.import_fow_data.warning_for_import_multiple_data_md",
                      ),
                      markdown: true,
                    );
                  }
                  if (!context.mounted) return;
                  await _selectImportFile(context, ImportType.fow);
                },
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SettingsTileIcon extends StatelessWidget {
  const _SettingsTileIcon({required this.icon, this.yellow = false});

  final IconData icon;
  final bool yellow;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(right: 12),
      child: Container(
        width: 32,
        height: 32,
        decoration: BoxDecoration(
          color: yellow ? StyleConstants.softYellow : StyleConstants.softGreen,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Icon(
          icon,
          size: 17,
          color: yellow ? StyleConstants.deepYellow : StyleConstants.deepGreen,
        ),
      ),
    );
  }
}

class _SettingsPageHeader extends StatelessWidget {
  const _SettingsPageHeader();

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerLeft,
      child: Text(
        context.tr('settings.page_title'),
        style: AppTypography.pageTitle.copyWith(color: StyleConstants.inkColor),
      ),
    );
  }
}
