import 'package:flutter/material.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

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
  final Widget content;
  final List<DialogButton> buttons;

  const CommonDialog({
    super.key,
    required this.title,
    required this.content,
    this.buttons = const [],
  });

  @override
  Widget build(BuildContext context) {
    return PointerInterceptor(
      child: AlertDialog(
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(24),
        ),
        title: Text(title),
        content: SingleChildScrollView(child: content),
        actionsPadding: const EdgeInsets.fromLTRB(24, 0, 24, 16),
        actions: buttons.map((button) {
          return FilledButton(
            onPressed: button.onPressed,
            style: FilledButton.styleFrom(
              backgroundColor: button.backgroundColor,
              foregroundColor: button.textColor,
            ),
            child: Text(button.text),
          );
        }).toList(),
      ),
    );
  }
}
