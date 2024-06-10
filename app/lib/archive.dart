import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:share_plus/share_plus.dart';
import 'import_data.dart';

class ArchiveUiBody extends StatelessWidget {
  const ArchiveUiBody({super.key});

  Future<String?> _selectData(ImportType? importType) async {
    FilePickerResult? result;
    if (importType != null && ImportType.fow == importType) {
      result = await FilePicker.platform
          .pickFiles(type: FileType.custom, allowedExtensions: ['zip']);
    } else {
      result = await FilePicker.platform.pickFiles(type: FileType.any);
    }
    if (result != null) {
      final path = result.files.single.path!;
      return path;
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ElevatedButton(
          onPressed: () async {
            String? path = await _selectData(null);
            if (path != null) {
              Navigator.push(context, MaterialPageRoute(
                builder: (context) {
                  return ImportDataPage(
                    path: path,
                  );
                },
              ));
            }
          },
          child: const Text("Import KML/GPX data"),
        ),
        ElevatedButton(
          onPressed: () async {
            String? path = await _selectData(ImportType.fow);
            if (path != null) {
              Navigator.push(context, MaterialPageRoute(
                builder: (context) {
                  return ImportDataPage(
                    path: path,
                    importType: ImportType.fow,
                  );
                },
              ));
            }
          },
          child: const Text("Import FOW data"),
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
            var result = await FilePicker.platform
                // TODO: have no idea why but the commented out line doesn't work
                // .pickFiles(type: FileType.custom, allowedExtensions: ['mldx']);
                .pickFiles(type: FileType.any);
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
