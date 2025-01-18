import 'dart:convert';
import 'package:flutter/material.dart';
import 'component/common_dialog.dart';

Future<bool> showInfoDialog(BuildContext context, String message,
    [showCancel = false]) async {
  List<Widget> messageBody =
      const LineSplitter().convert(message).map((s) => Text(s)).toList();

  var result = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
      return AlertDialog(
        title: const Text('Info'),
        content: SingleChildScrollView(
          child: ListBody(
            children: messageBody,
          ),
        ),
        actions: <Widget>[
          (showCancel)
              ? TextButton(
                  child: const Text('Cancel'),
                  onPressed: () {
                    Navigator.of(context).pop(false);
                  },
                )
              : Container(),
          TextButton(
            child: const Text('OK'),
            onPressed: () {
              Navigator.of(context).pop(true);
            },
          ),
        ],
      );
    },
  );

  return result ?? false;
}

Future<bool> showDeleteDialog(BuildContext context, String message) async {
  var result = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (BuildContext context) {
      return CommonDialog(
          title: "Delete",
          content: message,
          otherButtons: [
            DialogButton(
            text: "Yes",
            onPressed: (){Navigator.of(context).pop(true);},
            backgroundColor: Colors.red,
            textColor: Colors.white,
          )]
      );
    },
  );
  return result ?? false;
}
