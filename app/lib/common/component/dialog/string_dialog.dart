import 'dart:convert';

import 'package:easy_localization/easy_localization.dart';
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
  final bool showCancel;

  StringDialog({
    super.key,
    required this.title,
    required this.content,
    List<DialogButton>? buttons,
    this.customCancelButton,
    this.showCancel = false,
    this.markdown = false,
  }) : buttons = buttons ?? [];

  @override
  Widget build(BuildContext context) {
    final Widget messageBody = markdown
        ? MarkdownBody(
            data: content,
            onTapLink: (text, href, title) async {
              if (href == null) return;
              if (!await launchUrlString(
                href,
                mode: LaunchMode.externalApplication,
              )) {
                throw Exception('Could not launch url: $href');
              }
            },
          )
        : ListBody(
            children: LineSplitter()
                .convert(content)
                .map((line) => Text(line))
                .toList(),
          );

    final List<DialogButton> finalButtons = [...buttons];

    if (showCancel) {
      finalButtons.add(customCancelButton ??
          DialogButton(
            text: context.tr("common.cancel"),
            onPressed: () => Navigator.of(context).pop(),
          ));
    }

    return CommonDialog(
      title: title,
      content: messageBody,
      buttons: finalButtons,
    );
  }
}
