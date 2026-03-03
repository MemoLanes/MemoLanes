import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/body/settings/advanced_settings_page.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/body/settings/map_settings_page.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/component/tiles/label_tile_title.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
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

  void _loadVersion() async {
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
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
    //   allowedExtensions = ['kml', 'gpx'];
    // }
    final result = await FilePicker.platform.pickFiles(type: FileType.any);
    final path = result?.files.single.path;
    if (path != null && context.mounted) {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (context) {
            return ImportDataPage(path: path, importType: importType);
          },
        ),
      );
    }
  }

  Future<void> _loadNotificationStatus() async {
    setState(() {
      _isUnexpectedExitNotificationEnabled = MMKVUtil.getBool(
          MMKVKey.isUnexpectedExitNotificationEnabled,
          defaultValue: true);
    });
  }

  @override
  Widget build(BuildContext context) {
    var updateUrl = context.watch<UpdateNotifier>().updateUrl;
    var gpsManager = context.watch<GpsManager>();

    return MlSingleChildScrollView(
      padding: EdgeInsets.symmetric(vertical: 16.0),
      children: [
        // TODO: Enable this when we have user system.
        // CircleAvatar(
        //   backgroundColor: const Color(0xFFB6E13D),
        //   radius: 45.0,
        // ),
        // Padding(
        //   padding: EdgeInsets.symmetric(vertical: 16.0),
        //   child: Text(
        //     'Foo Bar',
        //     style: TextStyle(
        //       fontSize: 24.0,
        //       color: const Color(0xFFFFFFFF),
        //     ),
        //   ),
        // ),
        LabelTileTitle(
          label: context.tr("general.title"),
        ),
        LabelTile(
          label: context.tr("general.version.title"),
          position: LabelTilePosition.middle,
          trailing: updateUrl != null
              ? badges.Badge(
                  badgeStyle: badges.BadgeStyle(
                    shape: badges.BadgeShape.square,
                    borderRadius: BorderRadius.circular(5),
                    padding: const EdgeInsets.all(2),
                    badgeGradient: const badges.BadgeGradient.linear(
                      colors: [
                        Color(0xFFB7CC1F),
                        Color(0xFFB6E13D),
                        Color(0xFFB7CC1F),
                      ],
                    ),
                  ),
                  badgeContent: const Text(
                    'NEW',
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 8,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  child: LabelTileContent(
                    content: _version,
                  ),
                )
              : LabelTileContent(
                  content: _version,
                ),
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
          trailing: LabelTileContent(showArrow: true),
          onTap: () => Navigator.push(
            context,
            MaterialPageRoute(builder: (_) => const MapSettingsPage()),
          ),
        ),
        LabelTile(
          label: context.tr("general.advanced_settings.title"),
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(showArrow: true),
          onTap: () => Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) {
                return AdvancedSettingsPage();
              },
            ),
          ),
        ),
        LabelTileTitle(
          label: context.tr("data.title"),
        ),
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
          trailing: LabelTileContent(showArrow: true),
          onTap: () => _showImportDataCard(context),
        ),
        LabelTile(
          label: context.tr("data.export_data.export_all"),
          position: LabelTilePosition.bottom,
          onTap: () async {
            if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
              await showCommonDialog(
                context,
                context.tr("journey.stop_ongoing_journey"),
              );
              return;
            }
            var tmpDir = await getTemporaryDirectory();
            final now = DateTime.now();
            final timestamp = DateFormat('yyyy-MM-dd-HH-mm-ss').format(now);
            final filepath = "${tmpDir.path}/all-journeys-$timestamp.mldx";
            if (!context.mounted) return;
            await showLoadingDialog(
              context: context,
              asyncTask: api.generateFullArchive(targetFilepath: filepath),
            );
            if (!context.mounted) return;
            await showCommonExport(context, filepath, deleteFile: true);
          },
        ),
        LabelTileTitle(
          label: context.tr("settings.other"),
        ),
        LabelTile(
          label: context.tr("unexpected_exit_notification.setting_title"),
          position: LabelTilePosition.bottom,
          trailing: Switch(
            value: _isUnexpectedExitNotificationEnabled,
            onChanged: (value) async {
              final status = await Permission.notification.status;
              if (value) {
                if (!status.isGranted) {
                  setState(() {
                    _isUnexpectedExitNotificationEnabled = false;
                  });

                  if (!context.mounted) return;
                  await showCommonDialog(
                    context,
                    context.tr(
                        "unexpected_exit_notification.notification_permission_denied"),
                  );
                  Geolocator.openAppSettings();
                  return;
                }
              }
              MMKVUtil.putBool(
                  MMKVKey.isUnexpectedExitNotificationEnabled, value);
              setState(() {
                _isUnexpectedExitNotificationEnabled = value;
              });
              if (gpsManager.recordingStatus == GpsRecordingStatus.recording) {
                if (!context.mounted) return;
                await showCommonDialog(
                    context,
                    context.tr(
                      "unexpected_exit_notification.change_affect_next_time",
                    ));
              }
            },
          ),
        ),
        LabelTileTitle(
          label: context.tr("settings.about"),
        ),
        LabelTile(
          label: context.tr("privacy.name"),
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(rightIcon: Icons.open_in_new),
          onTap: () async {
            await launchUrlString(context.tr("privacy.url"),
                mode: LaunchMode.externalApplication);
          },
        ),
        LabelTile(
          label: context.tr("contact_us.title"),
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(rightIcon: Icons.arrow_forward_ios),
          onTap: () => Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) {
                return ContactUsPage();
              },
            ),
          ),
        ),
      ],
    );
  }

  void _showImportDataCard(BuildContext context) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("journey.import_mldx_data"),
            onTap: () async {
              // TODO: FilePicker is weird and `allowedExtensions` does not really work.
              // https://github.com/miguelpruivo/flutter_file_picker/wiki/FAQ
              var result = await FilePicker.platform.pickFiles(
                type: FileType.any,
              );
              if (!context.mounted) return;
              if (result != null) {
                var path = result.files.single.path;
                if (path != null) {
                  await importMldx(context, path);
                }
              }
            },
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.middle,
            label: context.tr("journey.import_kml_gpx_data"),
            onTap: () async {
              _selectImportFile(context, ImportType.gpxOrKml);
            },
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("journey.import_fog_of_world_data"),
            onTap: () async {
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
    );
  }
}
