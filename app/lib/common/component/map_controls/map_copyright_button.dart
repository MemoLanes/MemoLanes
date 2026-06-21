import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/utils.dart';

class MapCopyrightButton extends StatelessWidget {
  final String textMarkdown;

  static const double iconSize = 14;
  static const double contentPadding = 4;
  static const double bottomGap = 5;
  static const double trailingGap = 5;
  static const double navBarSpacing = 5;
  static const double buttonSize = iconSize + contentPadding * 2;

  const MapCopyrightButton({
    super.key,
    required this.textMarkdown,
  });

  @override
  Widget build(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);

    final padding = mediaQuery.viewPadding;
    final cornerInset = horizontalInsetFromBottomCorner(
      screenCornerRadius?.bottomRight,
      bottomInset: bottomGap,
      fallbackInset: 8,
    );

    return Align(
      alignment: Alignment.bottomRight,
      child: Padding(
        padding: EdgeInsets.only(
          right: padding.right + trailingGap + cornerInset,
          bottom: bottomGap,
        ),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () {
            showCommonDialog(context, textMarkdown,
                title: context.tr("home.map_data_source_copyright_title"),
                markdown: true);
          },
          child: Container(
            padding: const EdgeInsets.all(contentPadding),
            decoration: BoxDecoration(
              color: Colors.black.withValues(alpha: 0.45),
              shape: BoxShape.circle,
            ),
            child: const Icon(
              Icons.info_outline,
              size: iconSize,
              color: Colors.white,
            ),
          ),
        ),
      ),
    );
  }
}
