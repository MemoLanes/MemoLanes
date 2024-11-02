import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:memolanes/utils.dart';
import 'package:path_provider/path_provider.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:url_launcher/url_launcher.dart';

import 'import_data.dart';

class SettingsBody extends StatefulWidget {
  const SettingsBody({super.key});

  @override
  State<SettingsBody> createState() => _SettingsBodyState();
}

class _SettingsBodyState extends State<SettingsBody> {
  _launchUrl(String updateUrl) async {
    final url = Uri.parse(updateUrl);
    if (await canLaunchUrl(url)) {
      await launchUrl(url, mode: LaunchMode.externalApplication);
    } else {
      throw 'Could not launch $updateUrl';
    }
  }

  Future<void> _selectImportFile(
      BuildContext context, ImportType importType) async {
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
      Navigator.push(context, MaterialPageRoute(builder: (context) {
        return ImportDataPage(
          path: path,
          importType: importType,
        );
      }));
    }
  }

  @override
  Widget build(BuildContext context) {
    var updateUrl = context.watch<UpdateNotifier>().updateUrl;
    var gpsRecordingState = context.watch<GpsRecordingState>();

    return Column(
      children: [
        ElevatedButton(
          onPressed: () async {
            _selectImportFile(context, ImportType.gpxOrKml);
          },
          child: const Text("Import KML/GPX data"),
        ),
        ElevatedButton(
          onPressed: () async {
            await showInfoDialog(context,
                "This is an experimental feature and only supports zip compressed Fog of World Sync folder.\n\nPlease try not to import large amount of data or multiple datasets. A better import tool will be released in the future.");
            if (!context.mounted) return;
            await _selectImportFile(context, ImportType.fow);
          },
          child: const Text("Import FoW data"),
        ),
        ElevatedButton(
          onPressed: () async {
            if (gpsRecordingState.status != GpsRecordingStatus.none) {
              await showInfoDialog(context,
                  "Please stop the current ongoing journey before archiving.");
              return;
            }
            var tmpDir = await getTemporaryDirectory();
            var ts = DateTime.now().millisecondsSinceEpoch;
            var filepath = "${tmpDir.path}/${ts.toString()}.mldx";
            await generateFullArchive(targetFilepath: filepath);
            await Share.shareXFiles([XFile(filepath)]);
            try {
              var file = File(filepath);
              await file.delete();
            } catch (e) {
              print(e);
              // don't care about error
            }
          },
          child: const Text("Archive all (mldx file)"),
        ),
        ElevatedButton(
          onPressed: () async {
            if (gpsRecordingState.status != GpsRecordingStatus.none) {
              await showInfoDialog(context,
                  "Please stop the current ongoing journey before deleting all journeys.");
              return;
            }
            if (!await showInfoDialog(
                context,
                "This will delete all journeys in this app. Are you sure?",
                true)) {
              return;
            }
            try {
              await deleteAllJourneys();
              if (context.mounted) {
                await showInfoDialog(context, "All journeys are deleted.");
              }
            } catch (e) {
              if (context.mounted) {
                await showInfoDialog(context, e.toString());
              }
            }
          },
          child: const Text("Delete all journeys"),
        ),
        ElevatedButton(
          onPressed: () async {
            // TODO: FilePicker is weird and `allowedExtensions` does not really work.
            // https://github.com/miguelpruivo/flutter_file_picker/wiki/FAQ
            var result =
                await FilePicker.platform.pickFiles(type: FileType.any);
            if (result != null) {
              var path = result.files.single.path;
              if (path != null) {
                try {
                  await importArchive(mldxFilePath: path);
                } catch (e) {
                  if (context.mounted) {
                    await showInfoDialog(context, e.toString());
                  }
                }
              }
            }
          },
          child: const Text("Import archive (mldx file)"),
        ),
        ElevatedButton(
          onPressed: () async {
            var tmpDir = await getTemporaryDirectory();
            var ts = DateTime.now().millisecondsSinceEpoch;
            var filepath = "${tmpDir.path}/${ts.toString()}.zip";
            await exportLogs(targetFilePath: filepath);
            await Share.shareXFiles([XFile(filepath)]);
            try {
              var file = File(filepath);
              await file.delete();
            } catch (e) {
              print(e);
              // don't care about error
            }
          },
          child: const Text("Export Logs"),
        ),
        if (updateUrl != null)
          ElevatedButton(
            onPressed: () async {
              _launchUrl(updateUrl);
            },
            child: const Text(
              "Update",
              style: TextStyle(color: Colors.red),
            ),
          ),
        Text(
          "Version: ${shortCommitHash()}",
          style: const TextStyle(
            fontSize: 12.0,
            fontWeight: FontWeight.normal,
            color: Colors.black87,
            fontStyle: FontStyle.normal,
            decoration: TextDecoration.none,
          ),
        ),
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
