import 'dart:convert';

import 'package:flutter/material.dart';

Future<void> showInfoDialog(BuildContext context, String message) async {
  List<Widget> messageBody =
      const LineSplitter().convert(message).map((s) => Text(s)).toList();

  return showDialog<void>(
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
          TextButton(
            child: const Text('OK'),
            onPressed: () {
              Navigator.of(context).pop();
            },
          ),
        ],
      );
    },
  );
}
