import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'dart:ui';

class CommonDialog extends StatelessWidget {
  final String title;
  final String content;
  final VoidCallback? onConfirm;

  const CommonDialog({
    super.key,
    required this.title,
    required this.content,
    required this.onConfirm
  });

  @override
  Widget build(BuildContext context) {
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
      actions: <Widget>[
        FilledButton(
          onPressed: () => Navigator.of(context).pop(),
          style: FilledButton.styleFrom(
            backgroundColor: const Color(0xFFB4EC51),
            foregroundColor: Colors.black,
          ),
          child: Text(context.tr('common.cancel')),
        ),
        FilledButton(
          onPressed: () {
            if (onConfirm != null) {
              onConfirm!();
            }
            Navigator.of(context).pop();
          },
          style: FilledButton.styleFrom(
            backgroundColor: Colors.red,
            foregroundColor: Colors.white,
          ),
          child: Text(context.tr('common.end')),
        ),
      ],
    );
  }
}
