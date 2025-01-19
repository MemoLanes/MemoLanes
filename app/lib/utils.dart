import 'package:flutter/material.dart';

import 'component/common_dialog.dart';

Future<bool> showCommonDialog(BuildContext context, String message,
    {showCancel = false,
    title = "Info",
    confirmText = "OK",
    confirmGroundColor = const Color(0xFFB4EC51),
    confirmTextColor = Colors.black}) async {
  var result = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
      return CommonDialog(
          title: title,
          content: message,
          showCancel: showCancel,
          otherButtons: [
            DialogButton(
              text: confirmText,
              onPressed: () {
                Navigator.of(context).pop(true);
              },
              backgroundColor: confirmGroundColor,
              textColor: confirmTextColor,
            )
          ]);
    },
  );
  return result ?? false;
}
