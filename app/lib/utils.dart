import 'package:flutter/material.dart';

import 'component/common_dialog.dart';

Future<bool> showInfoDialog(BuildContext context, String message,
    {showCancel = false, title = "Info", confirmText = "OK"}) async {
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
              backgroundColor: Colors.red,
              textColor: Colors.white,
            )
          ]);
    },
  );
  return result ?? false;
}
