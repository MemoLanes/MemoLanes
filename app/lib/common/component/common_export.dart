import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_file_saver/flutter_file_saver.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/basic_bottom_sheet.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:path/path.dart' as p;
import 'package:share_plus/share_plus.dart';

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

class CommonExportResult {
  const CommonExportResult.create(this.exportResult, this.filePath);

  final api.ExportResult exportResult;
  final String filePath;
}

typedef CommonExportFileBuilder = Future<CommonExportResult> Function(
  CommonExportFormat format,
);

Future<void> showCommonExportWithFormatPicker({
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
  final selectedFormat = await showAppDialog<CommonExportFormat>(
    context,
    barrierDismissible: false,
    child: _ExportFormatDialog(
      title: title,
      formats: formats,
      initialFormat: initialFormat,
    ),
  );

  if (selectedFormat == null || !context.mounted) return;

  final CommonExportResult exportResult;
  try {
    exportResult = await GlobalLoadingManager.instance.runWithLoading(
      () => exportFile(selectedFormat),
    );
  } catch (error, stack) {
    log.error('[export] Export failed: $error', stack);
    if (context.mounted) {
      await showCommonDialog(
        context,
        context.tr('data.export_data.error.unexpected'),
      );
    }
    return;
  }

  switch (exportResult.exportResult) {
    case api.ExportResult.succeed:
      {
        final filePath = exportResult.filePath;

        if (!context.mounted) {
          if (deleteFile) await _deleteExportFile(filePath);
          return;
        }

        await showCommonExport(
          context,
          filePath,
          deleteFile: deleteFile,
        );
      }
    case api.ExportResult.dataIsEmpty:
      {
        if (context.mounted) {
          await showCommonDialog(
            context,
            context
                .tr('data.export_data.error.no_track_data_for_selected_format'),
          );
        }
      }
  }
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

    final action = await showBasicCard<_PreparedExportAction>(
      context,
      title: context.tr('common.export'),
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

    return AppOptionTile(
      icon: option.icon,
      title: option.title,
      subtitle: option.description,
      selected: selected,
      trailing: AppOptionTileTrailing.selection,
      onTap: () => _selectFormat(format),
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
        color: StyleConstants.warningSurfaceColor.withValues(alpha: 0.72),
        borderRadius: BorderRadius.circular(10.0),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            Icons.info_outline,
            color: StyleConstants.warningInkColor,
            size: 18.0,
          ),
          const SizedBox(width: 8.0),
          Expanded(
            child: Text(
              context.tr('data.export_data.lossy_format_warning'),
              style: AppTypography.supporting,
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return AppDialogCard(
      title: widget.title,
      maxHeightFactor: 0.78,
      contentPadding: const EdgeInsets.fromLTRB(20, 14, 20, 18),
      actions: AppDialogActions(
        children: [
          AppButton(
            label: context.tr('common.cancel'),
            variant: AppButtonVariant.secondary,
            onPressed: () => Navigator.of(context).pop(),
          ),
          AppButton(
            label: context.tr('common.export'),
            onPressed: _submit,
          ),
        ],
      ),
      child: AnimatedSize(
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
                  style: AppTypography.sectionLabel.copyWith(
                    color: StyleConstants.mutedInkColor,
                  ),
                ),
              ),
              const SizedBox(height: 6.0),
              for (var i = 0; i < widget.formats.length; i++) ...[
                _buildFormatOption(widget.formats[i]),
                if (i < widget.formats.length - 1) const SizedBox(height: 8),
              ],
              _buildLossyWarning(),
            ],
          ),
        ),
      ),
    );
  }
}

enum _PreparedExportAction { save, share }

class _ExportActionSheetContent extends StatelessWidget {
  const _ExportActionSheetContent();

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        AppOptionTile(
          icon: Icons.save_alt_outlined,
          title: context.tr('common.save'),
          onTap: () => Navigator.of(context).pop(_PreparedExportAction.save),
        ),
        const SizedBox(height: 8),
        AppOptionTile(
          icon: Icons.ios_share_outlined,
          title: context.tr('common.share'),
          onTap: () => Navigator.of(context).pop(_PreparedExportAction.share),
        ),
      ],
    );
  }
}
