import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_file_saver/flutter_file_saver.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:path/path.dart' as p;
import 'package:share_plus/share_plus.dart';

class CommonExport extends StatefulWidget {
  final String filePath;

  const CommonExport({super.key, required this.filePath});

  @override
  State<CommonExport> createState() => _CommonExportState();
}

class _CommonExportState extends State<CommonExport> {
  bool _showExportDialog = false;

  @override
  void initState() {
    super.initState();

    if (Platform.isIOS) {
      _shareFile();
    } else {
      _showExportDialog = true;
    }
  }

  Future<void> _shareFile() async {
    await SharePlus.instance.share(
      ShareParams(files: [XFile(widget.filePath)]),
    );
    if (!mounted) return;
    Navigator.of(context).pop();
  }

  Future<void> _saveFile() async {
    final file = File(widget.filePath);
    // TODO: This is pretty inefficient, but I don't think `FlutterFileSaver`
    // provides other API.
    await FlutterFileSaver().writeFileAsBytes(
      fileName: p.basename(widget.filePath),
      bytes: await file.readAsBytes(),
    );
    if (!mounted) return;
    Navigator.of(context).pop();
  }

  Widget _buildIconButton({
    required IconData icon,
    required String label,
    required VoidCallback onPressed,
  }) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(
          icon: FaIcon(icon, size: 40),
          onPressed: onPressed,
        ),
        Text(label),
      ],
    );
  }

  Widget _buildExportDialog() {
    return AlertDialog(
      title: Text(context.tr("common.export")),
      content: Row(
        mainAxisAlignment: MainAxisAlignment.spaceAround,
        children: [
          _buildIconButton(
            icon: FontAwesomeIcons.floppyDisk,
            label: context.tr("common.save"),
            onPressed: _saveFile,
          ),
          _buildIconButton(
            icon: FontAwesomeIcons.shareFromSquare,
            label: context.tr("common.share"),
            onPressed: _shareFile,
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return _showExportDialog ? _buildExportDialog() : const SizedBox.shrink();
  }
}
