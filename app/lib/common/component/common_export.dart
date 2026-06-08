import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_file_saver/flutter_file_saver.dart';
import 'package:memolanes/common/component/basic_bottom_sheet.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:path/path.dart' as p;
import 'package:share_plus/share_plus.dart';

const Color _exportActionSheetBackgroundColor = Color(0xFF242424);

class CommonExportOption {
  const CommonExportOption({
    required this.extension,
    required this.icon,
    required this.title,
    required this.description,
    this.keepsCompleteData = false,
  });

  final String extension;
  final IconData icon;
  final String title;
  final String description;
  final bool keepsCompleteData;
}

enum CommonExportFormat {
  mldx,
  fwss,
  kml,
  gpx;

  CommonExportOption get option {
    return switch (this) {
      CommonExportFormat.mldx => CommonExportOption(
          extension: 'mldx',
          icon: Icons.archive_outlined,
          title: tr('data.export_data.format_mldx'),
          description: tr('data.export_data.format_mldx_desc'),
          keepsCompleteData: true,
        ),
      CommonExportFormat.fwss => CommonExportOption(
          extension: 'fwss',
          icon: Icons.public_outlined,
          title: tr('data.export_data.format_fwss'),
          description: tr('data.export_data.format_fwss_desc'),
        ),
      CommonExportFormat.kml => CommonExportOption(
          extension: 'kml',
          icon: Icons.map_outlined,
          title: tr('data.export_data.format_kml'),
          description: tr('data.export_data.format_kml_desc'),
        ),
      CommonExportFormat.gpx => CommonExportOption(
          extension: 'gpx',
          icon: Icons.route_outlined,
          title: tr('data.export_data.format_gpx'),
          description: tr('data.export_data.format_gpx_desc'),
        ),
    };
  }

  String get extension => option.extension;
}

typedef CommonExportFileBuilder = Future<String> Function(
  CommonExportFormat format,
);

Future<bool> showCommonExportWithFormatPicker({
  required BuildContext context,
  required String title,
  required List<CommonExportFormat> formats,
  required CommonExportFileBuilder exportFile,
  CommonExportFormat? defaultFormat,
  bool deleteFile = true,
}) async {
  assert(formats.isNotEmpty);

  final initialFormat = defaultFormat == null
      ? formats.first
      : formats.firstWhere(
          (format) => format == defaultFormat,
          orElse: () => formats.first,
        );
  final selectedFormat = await showDialog<CommonExportFormat>(
    context: context,
    barrierDismissible: false,
    builder: (_) => _ExportFormatDialog(
      title: title,
      formats: formats,
      initialFormat: initialFormat,
    ),
  );

  if (selectedFormat == null || !context.mounted) return false;

  final filePath = await GlobalLoadingManager.instance.runWithLoading(
    () => exportFile(selectedFormat),
  );

  if (!context.mounted) {
    if (deleteFile) await _deleteExportFile(filePath);
    return false;
  }

  return showCommonExport(
    context,
    filePath,
    deleteFile: deleteFile,
  );
}

Future<bool> showCommonExport(
  BuildContext context,
  String filePath, {
  bool deleteFile = false,
}) async {
  final outerSharePositionOrigin = computeSharePositionOrigin(context);
  try {
    if (Platform.isIOS) {
      await _shareFile(filePath, outerSharePositionOrigin);
      return true;
    }

    final action = await showBasicBottomSheet<_PreparedExportAction>(
      context,
      showTitle: false,
      contentPadding: EdgeInsets.only(
        left: 20.0,
        right: 20.0,
        bottom: 12.0 + MediaQuery.of(context).padding.bottom,
      ),
      barrierColor: const Color(0x99000000),
      backgroundColor: _exportActionSheetBackgroundColor,
      child: const _ExportActionSheetContent(),
    );

    if (action == null || !context.mounted) return false;
    switch (action) {
      case _PreparedExportAction.save:
        await _saveFile(filePath);
        return true;
      case _PreparedExportAction.share:
        await _shareFile(filePath, computeSharePositionOrigin(context));
        return true;
    }
  } finally {
    if (deleteFile) {
      await _deleteExportFile(filePath);
    }
  }
}

Rect computeSharePositionOrigin(BuildContext context) {
  final box = context.findRenderObject() as RenderBox?;
  if (box == null) {
    return Rect.zero;
  } else {
    return box.localToGlobal(Offset.zero) & box.size;
  }
}

Future<void> _shareFile(String filePath, Rect sharePositionOrigin) {
  return SharePlus.instance.share(
    ShareParams(
      files: [XFile(filePath)],
      sharePositionOrigin: sharePositionOrigin,
    ),
  );
}

Future<void> _saveFile(String filePath) async {
  final file = File(filePath);
  // TODO: This is pretty inefficient, but I don't think `FlutterFileSaver`
  // provides other API.
  await FlutterFileSaver().writeFileAsBytes(
    fileName: p.basename(filePath),
    bytes: await file.readAsBytes(),
  );
}

Future<void> _deleteExportFile(String filePath) async {
  try {
    final file = File(filePath);
    if (await file.exists()) {
      await file.delete();
    }
  } catch (e, stack) {
    debugPrint('Failed to delete file: $e\n$stack');
  }
}

class _ExportFormatDialog extends StatefulWidget {
  const _ExportFormatDialog({
    required this.title,
    required this.formats,
    required this.initialFormat,
  });

  final String title;
  final List<CommonExportFormat> formats;
  final CommonExportFormat initialFormat;

  @override
  State<_ExportFormatDialog> createState() => _ExportFormatDialogState();
}

class _ExportFormatDialogState extends State<_ExportFormatDialog> {
  late CommonExportFormat _selectedFormat;

  @override
  void initState() {
    super.initState();
    _selectedFormat = widget.initialFormat;
  }

  void _selectFormat(CommonExportFormat value) {
    setState(() {
      _selectedFormat = value;
    });
  }

  void _submit() {
    Navigator.of(context).pop(_selectedFormat);
  }

  Widget _buildFormatOption(CommonExportFormat format) {
    final option = format.option;
    final selected = _selectedFormat == format;

    return Semantics(
      button: true,
      selected: selected,
      child: InkWell(
        onTap: () => _selectFormat(format),
        borderRadius: BorderRadius.circular(12.0),
        child: Container(
          padding: const EdgeInsets.symmetric(vertical: 10.0, horizontal: 4.0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Icon(
                option.icon,
                color: selected
                    ? StyleConstants.defaultColor
                    : const Color(0x99FFFFFF),
              ),
              const SizedBox(width: 12.0),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(option.title),
                    const SizedBox(height: 2.0),
                    Text(
                      option.description,
                      style: const TextStyle(
                        color: Color(0x99FFFFFF),
                        fontSize: 13.0,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 12.0),
              Icon(
                selected
                    ? Icons.radio_button_checked
                    : Icons.radio_button_unchecked,
                color: selected
                    ? StyleConstants.defaultColor
                    : const Color(0x99FFFFFF),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildLossyWarning() {
    if (_selectedFormat.option.keepsCompleteData) {
      return const SizedBox.shrink();
    }

    return AnimatedContainer(
      duration: const Duration(milliseconds: 180),
      curve: Curves.easeOut,
      margin: const EdgeInsets.only(top: 6.0),
      padding: const EdgeInsets.all(10.0),
      decoration: BoxDecoration(
        color: const Color(0x22FFC107),
        borderRadius: BorderRadius.circular(10.0),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(
            Icons.info_outline,
            color: Color(0xFFFFC107),
            size: 18.0,
          ),
          const SizedBox(width: 8.0),
          Expanded(
            child: Text(
              context.tr('data.export_data.lossy_format_warning'),
              style: const TextStyle(fontSize: 13.0),
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
      ),
      title: Text(widget.title),
      content: AnimatedSize(
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeOut,
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: Text(
                  context.tr('data.export_data.format_section_title'),
                  style: const TextStyle(
                    color: Color(0x99FFFFFF),
                    fontSize: 13.0,
                  ),
                ),
              ),
              const SizedBox(height: 6.0),
              for (final format in widget.formats) _buildFormatOption(format),
              _buildLossyWarning(),
            ],
          ),
        ),
      ),
      actionsPadding: const EdgeInsets.fromLTRB(24, 0, 24, 16),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(context.tr('common.cancel')),
        ),
        FilledButton(
          onPressed: _submit,
          style: FilledButton.styleFrom(
            backgroundColor: StyleConstants.defaultColor,
            foregroundColor: Colors.black,
          ),
          child: Text(context.tr('common.export')),
        ),
      ],
    );
  }
}

enum _PreparedExportAction { save, share }

class _ExportActionSheetContent extends StatelessWidget {
  const _ExportActionSheetContent();

  Widget _buildActionButton({
    required BuildContext context,
    required IconData icon,
    required String label,
    required _PreparedExportAction action,
  }) {
    return InkWell(
      onTap: () => Navigator.of(context).pop(action),
      borderRadius: BorderRadius.circular(16.0),
      child: SizedBox(
        width: 96.0,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 10.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 56.0,
                height: 56.0,
                decoration: const BoxDecoration(
                  color: Color(0x1AFFFFFF),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  icon,
                  color: StyleConstants.defaultColor,
                  size: 26.0,
                ),
              ),
              const SizedBox(height: 8.0),
              Text(
                label,
                textAlign: TextAlign.center,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
          children: [
            _buildActionButton(
              context: context,
              icon: Icons.save_alt_outlined,
              label: context.tr('common.save'),
              action: _PreparedExportAction.save,
            ),
            _buildActionButton(
              context: context,
              icon: Icons.ios_share_outlined,
              label: context.tr('common.share'),
              action: _PreparedExportAction.share,
            ),
          ],
        ),
      ],
    );
  }
}
