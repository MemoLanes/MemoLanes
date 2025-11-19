import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:url_launcher/url_launcher_string.dart';

import 'common_dialog.dart';

class StringDialog extends StatelessWidget {
  final String title;
  final String content;
  final bool markdown;
  final List<DialogButton> buttons;

  final DialogButton? customCancelButton;

  StringDialog(
      {super.key,
      required this.title,
      required this.content,
      List<DialogButton>? buttons,
      this.customCancelButton,
      this.markdown = false})
      : buttons = buttons ?? [];

  @override
  Widget build(BuildContext context) {
    Widget messageBody = switch (markdown) {
      false => ListBody(
          children: const LineSplitter()
              .convert(content)
              .map((s) => Text(
                    s,
                  ))
              .toList()),
      true => MarkdownBody(
          data: content,
          onTapLink: (text, href, title) async {
            if (href == null) {
              return;
            }
            if (!await launchUrlString(href,
                mode: LaunchMode.externalApplication)) {
              throw Exception('Could not launch url: $href');
            }
          },
        )
    };

    return CommonDialog(
      title: title,
      content: messageBody,
      buttons: buttons,
    );
  }
}
