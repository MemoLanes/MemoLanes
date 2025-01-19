import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';

import 'component/common_dialog.dart';

Future<bool> showCommonDialog(BuildContext context, String message,
    {showCancel = false,
    title,
    confirmText,
    confirmGroundColor,
    confirmTextColor = Colors.black}) async {
  const defaultGroundColor = Color(0xFFB4EC51);
  confirmText = confirmText ?? context.tr("common.confirm");
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
    if (showCancel)
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
          showCancel: showCancel,
          buttons: allButtons);
    },
  );
  return result ?? false;
}
