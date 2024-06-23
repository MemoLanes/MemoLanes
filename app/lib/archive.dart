import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:share_plus/share_plus.dart';
import 'package:url_launcher/url_launcher.dart';

class ArchiveUiBody extends StatelessWidget {
  const ArchiveUiBody({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: HomeScreen(),
    );
  }
}

class HomeScreen extends StatefulWidget {
  @override
  _HomeScreenState createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  bool _isButtonVisible = false;
  String? _url;

  _launchUrl(url) async {
    final url_ = Uri.parse(url);
    if (await canLaunchUrl(url_)) {
      await launchUrl(url_);
    } else {
      throw 'Could not launch $url';
    }
  }

  _newVersionNotification(url) async {
    setState(() {
      if (url != null) {
        _isButtonVisible = true;
        _url = url;
      } else {
        _isButtonVisible = false;
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ElevatedButton(
          onPressed: () async {
            var result = await FilePicker.platform
                .pickFiles(type: FileType.custom, allowedExtensions: ['zip']);
            if (result != null) {
              var path = result.files.single.path;
              if (path != null) {
                await importFowData(zipFilePath: path);
              }
            }
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
        if (_isButtonVisible)
          ElevatedButton(
            onPressed: () async {
              _launchUrl(_url);
            },
            child: const Text("Update"),
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
