import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/component/common_export.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

Future<bool> showCommonDialog(BuildContext context, String message,
    {hasCancel = false,
    title,
    confirmButtonText,
    cancelButtonText,
    confirmGroundColor,
    confirmTextColor = Colors.black,
    markdown = false}) async {
  const defaultGroundColor = Color(0xFFB4EC51);
  confirmButtonText = confirmButtonText ?? context.tr("common.ok");
  cancelButtonText = cancelButtonText ?? context.tr("common.cancel");
  title = title ?? context.tr("common.info");
  confirmGroundColor = confirmGroundColor ?? defaultGroundColor;
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
          backgroundColor: confirmGroundColor == defaultGroundColor
              ? Colors.grey
              : defaultGroundColor,
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
  required BuildContext context,
  required Future<T> asyncTask,
}) async {
  var taskCompleteEarly = false;
  asyncTask.whenComplete(() {
    taskCompleteEarly = true;
  });

  // Do not show the loading dialog if the task is fast
  await Future.delayed(const Duration(milliseconds: 200));
  if (taskCompleteEarly) return asyncTask;
  if (!context.mounted) return asyncTask;

  BuildContext? dialogContext;
  showDialog(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
      dialogContext = context;
      return Padding(
        padding: const EdgeInsets.only(top: 10),
        child: Center(
          child: Container(
            width: 80,
            height: 80,
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(16),
            ),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                CircularProgressIndicator(
                  strokeWidth: 3.0,
                ),
              ],
            ),
          ),
        ),
      );
    },
  );

  await Future.delayed(const Duration(milliseconds: 50));

  T result;
  try {
    await WakelockPlus.enable();
    result = await asyncTask;
  } finally {
    await WakelockPlus.disable();
    var context = dialogContext;
    if (context != null) {
      if (context.mounted) {
        Navigator.of(context).pop();
      }
    }
  }
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
