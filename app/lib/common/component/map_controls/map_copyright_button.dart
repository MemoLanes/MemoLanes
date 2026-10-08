import 'dart:ui' show lerpDouble;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/theme/app_colors.dart';

class MapCopyrightButton extends StatelessWidget {
  final String textMarkdown;

  static const double iconSize = 14;
  static const double contentPadding = 4;
  static const double buttonOpacity = 0.70;
  static const double buttonSize = iconSize + contentPadding * 2;

  const MapCopyrightButton({super.key, required this.textMarkdown});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () {
        showCommonDialog(
          context,
          textMarkdown,
          title: context.tr("home.map_data_source_copyright_title"),
          markdown: true,
        );
      },
      child: Opacity(
        opacity: buttonOpacity,
        child: Container(
          padding: const EdgeInsets.all(contentPadding),
          decoration: BoxDecoration(
            color: context.appColors.shadowColor.withValues(
              alpha: lerpDouble(0.45, 0.58, context.appColors.darkProgress)!,
            ),
            shape: BoxShape.circle,
          ),
          child: Icon(
            Icons.info_outline,
            size: iconSize,
            color: Color.lerp(
              Colors.white,
              context.appColors.onStrongColor,
              context.appColors.darkProgress,
            )!,
          ),
        ),
      ),
    );
  }
}
