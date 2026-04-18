import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/body/settings/mldx_import_page.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/common/log.dart';

final _naiveDateFormat = DateFormat('yyyy-MM-dd');

NaiveDate dateTimeToNaiveDate(DateTime dateTime) =>
    naiveDateOfString(str: _naiveDateFormat.format(dateTime));

DateTime naiveDateToDateTime(NaiveDate naiveDate) =>
    _naiveDateFormat.parse(naiveDateToString(date: naiveDate));

Future<bool> showCommonDialog(BuildContext context, String message,
    {hasCancel = false,
    title,
    confirmButtonText,
    cancelButtonText,
    confirmGroundColor,
    confirmTextColor = Colors.black,
    markdown = false}) async {
  confirmButtonText = confirmButtonText ?? context.tr("common.ok");
  cancelButtonText = cancelButtonText ?? context.tr("common.cancel");
  title = title ?? context.tr("common.info");
  confirmGroundColor = confirmGroundColor ?? StyleConstants.defaultColor;
  final List<DialogButton> allButtons = [
    DialogButton(
      text: confirmButtonText,
      onPressed: () {
        Navigator.of(context).pop(true);
      },
      backgroundColor: confirmGroundColor,
      textColor: confirmTextColor,
    ),
    if (hasCancel)
      DialogButton(
          text: cancelButtonText,
          backgroundColor: confirmGroundColor == StyleConstants.defaultColor
              ? Colors.grey
              : StyleConstants.defaultColor,
          onPressed: () {
            Navigator.of(context).pop(false);
          })
  ];

  var result = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
      return CommonDialog(
        title: title,
        content: message,
        showCancel: hasCancel,
        buttons: allButtons,
        markdown: markdown,
      );
    },
  );
  return result ?? false;
}

Future<T> showLoadingDialog<T>({
  required Future<T> asyncTask,
}) async {
  final result = await GlobalLoadingManager.instance.runWithLoading<T>(
    () => asyncTask,
  );
  return result;
}

Future<bool> showCommonExport(BuildContext context, String filePath,
    {bool deleteFile = false}) async {
  final outerSharePositionOrigin = computeSharePositionOrigin(context);
  final dialogResult = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (_) => CommonExport(
        filePath: filePath, outerSharePositionOrigin: outerSharePositionOrigin),
  );

  if (deleteFile) {
    try {
      final file = File(filePath);
      if (await file.exists()) {
        await file.delete();
      }
    } catch (e, stack) {
      debugPrint('Failed to delete file: $e\n$stack');
    }
  }

  return dialogResult ?? false;
}

void showBasicCard(
  BuildContext context, {
  required Widget child,
  bool showHandle = true,
}) {
  showModalBottomSheet(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    builder: (context) {
      return Container(
        decoration: BoxDecoration(
          color: Colors.black,
          borderRadius: BorderRadius.only(
            topLeft: Radius.circular(16.0),
            topRight: Radius.circular(16.0),
          ),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(
              padding: EdgeInsets.symmetric(vertical: 12.0),
              child: Offstage(
                offstage: !showHandle,
                child: Center(
                  child: CustomPaint(
                    size: Size(40.0, 4.0),
                    painter: LinePainter(
                      color: const Color(0xFFB5B5B5),
                    ),
                  ),
                ),
              ),
            ),
            child,
          ],
        ),
      );
    },
  );
}

Future<void> importMldx(BuildContext context, String path) async {
  try {
    final (mldxFile, preview) = await showLoadingDialog(
      asyncTask: (() async {
        final mldxFile = await OpaqueMldxReader.open(mldxFilePath: path);
        final preview = await mldxFile.analyze();
        return (mldxFile, preview);
      })(),
    );
    if (!context.mounted) return;
    final unchangedCount = preview
        .where((j) => j.$2 == MldxJourneyImportAnalyzeResult.unchanged)
        .length;
    final importableCount = preview
        .where((j) => j.$2 != MldxJourneyImportAnalyzeResult.unchanged)
        .length;

    // If everything is skipped, end the flow here.
    if (importableCount == 0 && unchangedCount > 0) {
      await showCommonDialog(
        context,
        context.tr(
          'import.mldx_preview.all_skipped',
          args: ['$unchangedCount'],
        ),
      );
      return;
    }
    await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (context) => MldxImportPage(
          journeys: preview,
          mldxReader: mldxFile,
        ),
      ),
    );
  } catch (error) {
    if (context.mounted) {
      await showCommonDialog(context, context.tr("import.parsing_failed"));
      log.error("[import_data] Data parsing failed $error");
    }
  }
}
