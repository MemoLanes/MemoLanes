import 'dart:developer';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:project_dv/src/rust/api/api.dart';

class ImportUI extends StatelessWidget {
  const ImportUI({super.key});

  @override
  Widget build(BuildContext context) {
    return (ElevatedButton(
      onPressed: () async {
        var result = await FilePicker.platform
            .pickFiles(type: FileType.custom, allowedExtensions: ['zip']);
        if (result != null) {
          var path = result.files.single.path;
          if (path != null) {
            log(path);
            await importFowData(zipFilePath: path);
          }
        }
      },
      child: const Text("Import FoW data"),
    ));
  }
}
