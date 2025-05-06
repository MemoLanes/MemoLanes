import 'dart:io';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_file_saver/flutter_file_saver.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:share_plus/share_plus.dart';
import 'package:path/path.dart' as p;

class CommonExport extends StatelessWidget {
  final String filePath;

  const CommonExport({super.key, required this.filePath});

  Future<void> _shareFile(BuildContext context, String filepath) async {
    await Share.shareXFiles([XFile(filepath)]);
    _deleteFile(filepath);
    if (context.mounted) Navigator.of(context).pop();
  }

  Future<void> _deleteFile(String filepath) async {
    try {
      await File(filepath).delete();
    } catch (e) {
      debugPrint('Failed to delete file: $e');
    }
  }

  Future<void> _saveFile(BuildContext context, String filepath) async {
    final file = File(filepath);
    // TODO: This is pretty inefficient, but I don't think `FlutterFileSaver`
    // provides other API.
    await FlutterFileSaver().writeFileAsBytes(
      fileName: p.basename(filePath),
      bytes: await file.readAsBytes(),
    );
    _deleteFile(filepath);
    if (context.mounted) Navigator.of(context).pop();
  }

  Widget _buildExportDialog(BuildContext context, String filepath) {
    return AlertDialog(
      title: Text(context.tr("common.export")),
      content: Row(
        mainAxisAlignment: MainAxisAlignment.spaceAround,
        children: [
          _buildIconButton(
            icon: FontAwesomeIcons.floppyDisk,
            label: context.tr("common.save"),
            onPressed: () => _saveFile(context, filepath),
          ),
          _buildIconButton(
            icon: FontAwesomeIcons.shareFromSquare,
            label: context.tr("common.share"),
            onPressed: () {
              _shareFile(context, filepath);
            },
          ),
        ],
      ),
    );
  }

  Widget _buildIconButton(
      {required IconData icon,
      required String label,
      required VoidCallback onPressed}) {
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(icon: FaIcon(icon, size: 40), onPressed: onPressed),
        Text(label),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (Platform.isAndroid) {
      return _buildExportDialog(context, filePath);
    } else {
      _shareFile(context, filePath);
      return const SizedBox.shrink();
    }
  }
}
