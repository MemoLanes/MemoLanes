import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/advanced_settings.dart';
import 'package:memolanes/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/component/tiles/label_tile.dart';
import 'package:memolanes/component/tiles/label_tile_content.dart';
import 'package:memolanes/component/tiles/label_tile_title.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:memolanes/import_data.dart';
import 'package:memolanes/preferences_manager.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

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

  _loadVersion() async {
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    setState(() {
      _version =
          '${packageInfo.version} (${packageInfo.buildNumber}) [${api.shortCommitHash()}]';
    });
  }

  _launchUrl(String updateUrl) async {
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
    final status =
        await PreferencesManager.getUnexpectedExitNotificationStatus();
    setState(() {
      _isUnexpectedExitNotificationEnabled = status;
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
          label: context.tr("general.advance_settings.title"),
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(showArrow: true),
          onTap: () => Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) {
                return AdvancedSettingsScreen();
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
          onTap: () => showImportDataCard(
            context,
            onLabelTaped: (name) async {
              switch (name) {
                case 'MLDX':
                  // TODO: FilePicker is weird and `allowedExtensions` does not really work.
                  // https://github.com/miguelpruivo/flutter_file_picker/wiki/FAQ
                  var result = await FilePicker.platform.pickFiles(
                    type: FileType.any,
                  );
                  if (!context.mounted) return;
                  if (result != null) {
                    var path = result.files.single.path;
                    if (path != null) {
                      try {
                        await showLoadingDialog(
                          context: context,
                          asyncTask: api.importArchive(mldxFilePath: path),
                        );
                        if (context.mounted) {
                          await showCommonDialog(
                            context,
                            "Import succeeded!",
                            title: "Success",
                          );
                        }
                      } catch (e) {
                        if (context.mounted) {
                          await showCommonDialog(context, e.toString());
                        }
                      }
                    }
                  }
                  break;
                case 'KML/GPX':
                  _selectImportFile(context, ImportType.gpxOrKml);
                  break;
                case 'FOG_OF_WORLD':
                  await showCommonDialog(
                    context,
                    context.tr("import_fow_data.description_md"),
                    markdown: true,
                  );
                  if (await api.containsBitmapJourney()) {
                    if (!context.mounted) return;
                    await showCommonDialog(
                      context,
                      context.tr(
                        "import_fow_data.warning_for_import_multiple_data_md",
                      ),
                      markdown: true,
                    );
                  }
                  if (!context.mounted) return;
                  await _selectImportFile(context, ImportType.fow);
                  break;
              }
            },
          ),
        ),
        LabelTile(
          label: context.tr("data.export_data.export_all"),
          position: LabelTilePosition.middle,
          onTap: () async {
            if (gpsManager.recordingStatus != GpsRecordingStatus.none) {
              await showCommonDialog(
                context,
                "Please stop the current ongoing journey before archiving.",
              );
              return;
            }
            var tmpDir = await getTemporaryDirectory();
            var ts = DateTime.now().millisecondsSinceEpoch;
            var filepath = "${tmpDir.path}/${ts.toString()}.mldx";
            if (!context.mounted) return;
            await showLoadingDialog(
              context: context,
              asyncTask: api.generateFullArchive(targetFilepath: filepath),
            );
            if (!context.mounted) return;
            await showCommonExport(context, filepath, deleteFile: true);
          },
        ),
        // TODO: Add about us / privacy policy / contact us / FAQ / suggestion ...
        LabelTileTitle(
          label: context.tr("other.title"),
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
              await PreferencesManager.setUnexpectedExitNotificationStatus(
                  value);
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
        SizedBox(height: 96.0),
      ],
    );
  }
}

class UpdateNotifier extends ChangeNotifier {
  String? updateUrl;

  void setUpdateUrl(String? url) {
    updateUrl = url;
    notifyListeners();
  }

  bool hasUpdateNotification() {
    return updateUrl != null;
  }
}
