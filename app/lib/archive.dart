import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:share_plus/share_plus.dart';
import 'import_data.dart';

class ArchiveUiBody extends StatelessWidget {
  const ArchiveUiBody({super.key});

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
            _selectImportFile(context, ImportType.fow);
          },
          child: const Text("Import FoW data"),
        ),
        ElevatedButton(
          onPressed: () async {
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
          child: const Text("Archive All"),
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
                await recoverFromArchive(zipFilePath: path);
              }
            }
          },
          child: const Text("Reset & Recover"),
        ),
        Text("Version: ${shortCommitHash()}"),
      ],
    );
  }
}
