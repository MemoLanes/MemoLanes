
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';

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
  final List<DialogButton> otherButtons;
  final bool cancelButton;
  final DialogButton? customCancelButton;

  CommonDialog({
    super.key,
    required this.title,
    required this.content,
    List<DialogButton>? otherButtons,
    bool? cancelButton,
    this.customCancelButton,
  }) : otherButtons = otherButtons ?? [],
        cancelButton = cancelButton ?? true
  ;

  @override
  Widget build(BuildContext context) {
    final List<DialogButton> allButtons = [
      ...otherButtons,
      if (cancelButton)
        customCancelButton ?? DialogButton(
          text: context.tr('common.cancel'),
          onPressed: (){Navigator.of(context).pop(false);}
        ),
    ];

    return AlertDialog(
      backgroundColor: Colors.white,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
      ),
      title: Text(
        title,
        style: const TextStyle(color: Colors.black),
      ),
      content: Text(
        content,
        style: const TextStyle(color: Colors.black54),
      ),
      actionsPadding: const EdgeInsets.fromLTRB(24, 0, 24, 16),
        actions: allButtons.map((button) {
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
        }).toList()
    );
  }
}

