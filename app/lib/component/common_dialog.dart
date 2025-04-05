import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:url_launcher/url_launcher_string.dart';

class DialogButton {
  final String text;
  final VoidCallback onPressed;
  final Color backgroundColor;
  final Color textColor;

  DialogButton({
    required this.text,
    VoidCallback? onPressed,
    Color? backgroundColor,
    Color? textColor,
  })  : backgroundColor = backgroundColor ?? const Color(0xFFB4EC51),
        textColor = textColor ?? Colors.black,
        onPressed = onPressed ?? (() => {});
}

class CommonDialog extends StatelessWidget {
  final String title;
  final String content;
  final bool markdown;
  final List<DialogButton> buttons;

  final DialogButton? customCancelButton;

  CommonDialog(
      {super.key,
      required this.title,
      required this.content,
      List<DialogButton>? buttons,
      bool? showCancel,
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
                    style: const TextStyle(color: Colors.black54),
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

    return PointerInterceptor(
        child: AlertDialog(
            backgroundColor: Colors.white,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(24),
            ),
            title: Text(
              title,
              style: const TextStyle(color: Colors.black),
            ),
            content: SingleChildScrollView(
              child: messageBody,
            ),
            actionsPadding: const EdgeInsets.fromLTRB(24, 0, 24, 16),
            actions: buttons.map((button) {
              return FilledButton(
                onPressed: () {
                  button.onPressed();
                },
                style: FilledButton.styleFrom(
                  backgroundColor: button.backgroundColor,
                  foregroundColor: button.textColor,
                ),
                child: Text(button.text),
              );
            }).toList()));
  }
}
