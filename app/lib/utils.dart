import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';

import 'component/common_dialog.dart';

Future<bool> showCommonDialog(BuildContext context, String message,
    {hasCancel = false,
    title,
    confirmText,
    confirmGroundColor,
    confirmTextColor = Colors.black}) async {
  const defaultGroundColor = Color(0xFFB4EC51);
  confirmText = confirmText ?? context.tr("common.ok");
  title = title ?? context.tr("common.info");
  confirmGroundColor = confirmGroundColor ?? defaultGroundColor;
  final List<DialogButton> allButtons = [
    DialogButton(
      text: confirmText,
      onPressed: () {
        Navigator.of(context).pop(true);
      },
      backgroundColor: confirmGroundColor,
      textColor: confirmTextColor,
    ),
    if (hasCancel)
      DialogButton(
          text: context.tr('common.cancel'),
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
          buttons: allButtons);
    },
  );
  return result ?? false;
}

Future<T?> showLoadingDialog<T>({
  required BuildContext context,
  required Future<T> asyncTask,
}) async {
  if (!context.mounted) return Future.value();

  T result;

  showDialog(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
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
  try {
    result = await asyncTask;
    return result;
  } finally {
    if (context.mounted) {
      Navigator.of(context).pop();
    }
  }
}
