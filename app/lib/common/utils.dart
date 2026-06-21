import 'package:easy_localization/easy_localization.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/body/settings/mldx_import_page.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/common/log.dart';
import 'package:screen_corner_radius/screen_corner_radius.dart';

final _naiveDateFormat = DateFormat('yyyy-MM-dd');
ScreenRadius? screenCornerRadius;

double horizontalInsetFromBottomCorner(
  double? radius, {
  required double bottomInset,
  required double fallbackInset,
}) {
  final cornerRadius = radius ?? 0.0;
  if (cornerRadius <= 0.0) return fallbackInset;

  final distanceIntoCorner =
      (cornerRadius - bottomInset).clamp(0.0, cornerRadius).toDouble();
  if (distanceIntoCorner <= 0.0) return 0.0;

  final inset = distanceIntoCorner * 0.45;
  return inset < fallbackInset ? fallbackInset : inset;
}

Future<void> initScreenCornerRadius() async {
  screenCornerRadius = await ScreenCornerRadius.get();
}

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
