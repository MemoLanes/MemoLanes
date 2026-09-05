import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:url_launcher/url_launcher_string.dart';

class DialogButton {
  const DialogButton({
    required this.text,
    required this.onPressed,
    this.variant = AppButtonVariant.primary,
  });

  final String text;
  final VoidCallback onPressed;
  final AppButtonVariant variant;
}

class CommonDialog extends StatelessWidget {
  const CommonDialog({
    super.key,
    required this.title,
    required this.content,
    this.buttons = const [],
    this.markdown = false,
  });

  final String title;
  final String content;
  final bool markdown;
  final List<DialogButton> buttons;

  @override
  Widget build(BuildContext context) {
    final messageBody = switch (markdown) {
      false => ListBody(
        children: const LineSplitter()
            .convert(content)
            .map(
              (line) => Text(
                line,
                style: AppTypography.body.copyWith(
                  color: StyleConstants.inkColor,
                ),
              ),
            )
            .toList(),
      ),
      true => MarkdownBody(
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
      ),
    };

    return AppDialogCard(
      title: title,
      maxHeightFactor: 0.78,
      contentPadding: const EdgeInsets.fromLTRB(20, 14, 20, 18),
      actions: buttons.isEmpty
          ? null
          : AppDialogActions(
              children: [
                for (final button in buttons)
                  AppButton(
                    label: button.text,
                    labelMaxLines: 2,
                    variant: button.variant,
                    onPressed: () {
                      AppHaptics.selection();
                      button.onPressed();
                    },
                  ),
              ],
            ),
      child: messageBody,
    );
  }
}
