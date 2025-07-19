import 'package:easy_localization/easy_localization.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/advanced_settings_screen.dart';
import 'package:memolanes/backup_data_screen.dart';
import 'package:memolanes/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/component/tiles/label_tile.dart';
import 'package:memolanes/component/tiles/label_tile_content.dart';
import 'package:memolanes/component/tiles/label_tile_title.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:memolanes/import_data.dart';
import 'package:memolanes/preferences_manager.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/utils.dart';
import 'package:path_provider/path_provider.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  bool _isUnexpectedExitNotificationEnabled = false;

  @override
  void initState() {
    super.initState();
    _loadNotificationStatus();
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
        CircleAvatar(
          backgroundColor: const Color(0xFFB6E13D),
          radius: 45.0,
        ),
        Padding(
          padding: EdgeInsets.symmetric(vertical: 16.0),
          child: Text(
            'Ryan Schnetzer',
            style: TextStyle(
              fontSize: 24.0,
              color: const Color(0xFFFFFFFF),
            ),
          ),
        ),
        LabelTileTitle(
          label: '通用',
        ),
        LabelTile(
          label: '版本信息',
          position: LabelTilePosition.middle,
          trailing: Row(
            children: [
              LabelTileContent(
                content: 'v1.5.3',
              ),
              Padding(
                padding: EdgeInsets.only(left: 4.0),
                child: Container(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.all(Radius.circular(12.0)),
                    border: Border.all(
                      color: const Color(0xFFFF0000),
                      width: 1,
                    ),
                  ),
                  child: Padding(
                    padding: EdgeInsets.all(2.0),
                    child: Text(
                      '有新版',
                      style: TextStyle(
                        color: const Color(0xFFFF0000),
                        fontSize: 10.0,
                      ),
                    ),
                  ),
                ),
              )
            ],
          ),
          onTap: () {},
        ),
        LabelTile(
          label: '高级设置',
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
          label: '数据',
        ),
        LabelTile(
          label: '数据备份',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(showArrow: true),
          onTap: () => Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) {
                return BackupDataScreen();
              },
            ),
          ),
        ),
        LabelTile(
          label: '数据导入',
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
                case '世界迷雾':
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
          label: '数据导出',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(showArrow: true),
          onTap: () => showExportDataCard(
            context,
            onLabelTaped: (name) async {
              /// TODO
              if (name != 'MLDX') {
                await showCommonDialog(
                  context,
                  "Still under development.",
                );
                return;
              }
              // MLDX
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
        ),
        LabelTile(
          label: '清除 App 数据',
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(showArrow: true),
          onTap: () {},
        ),
        LabelTileTitle(
          label: '关于我们',
        ),
        LabelTile(
          label: '个人隐私政策',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(showArrow: true),
          onTap: () {},
        ),
        LabelTile(
          label: 'App 开源项目使用',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(showArrow: true),
          onTap: () {},
        ),
        LabelTile(
          label: '联系开发者',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(showArrow: true),
          onTap: () {},
        ),
        LabelTile(
          label: 'FAQ',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(showArrow: true),
          onTap: () {},
        ),
        LabelTile(
          label: '建议',
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(showArrow: true),
          onTap: () {},
        ),
        LabelTileTitle(
          label: '其他',
        ),
        LabelTile(
          label: context.tr("unexpected_exit_notification.setting_title"),
          position: LabelTilePosition.middle,
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
        if (updateUrl != null) ...[
          LabelTile(
            label: 'Update',
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(showArrow: true),
            onTap: () => _launchUrl(updateUrl),
          ),
        ],
        LabelTile(
          label: "Version",
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(content: api.shortCommitHash()),
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
