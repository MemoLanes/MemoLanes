import 'dart:io';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_file_saver/flutter_file_saver.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:share_plus/share_plus.dart';
import 'package:path/path.dart' as p;

class CommonExport extends StatefulWidget {
  final String filePath;

  const CommonExport({super.key, required this.filePath});

  @override
  State<CommonExport> createState() => _CommonExportState();
}

class _CommonExportState extends State<CommonExport> {
  late final String filePath;

  @override
  void initState() {
    super.initState();
    filePath = widget.filePath;

    if (Platform.isIOS) {
      Future.microtask(() => _shareFile());
    }
  }

  Future<void> _shareFile() async {
    await Share.shareXFiles([XFile(filePath)]);
    if (!mounted) return;
    Navigator.of(context).pop();
  }

  Future<void> _saveFile() async {
    final file = File(filePath);
    // TODO: This is pretty inefficient, but I don't think `FlutterFileSaver`
    // provides other API.
    await FlutterFileSaver().writeFileAsBytes(
      fileName: p.basename(filePath),
      bytes: await file.readAsBytes(),
    );
    if (!mounted) return;
    Navigator.of(context).pop();
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

  @override
  Widget build(BuildContext context) {
    return Platform.isAndroid ? _buildExportDialog() : const SizedBox.shrink();
  }
}
