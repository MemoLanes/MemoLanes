import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/utils.dart';

class MapCopyrightButton extends StatelessWidget {
  final String textMarkdown;

  const MapCopyrightButton({
    super.key,
    required this.textMarkdown,
  });

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      bottom: true,
      child: Align(
        alignment: Alignment.bottomRight,
        child: Padding(
          padding: const EdgeInsets.only(right: 10, bottom: 10),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () {
              showCommonDialog(context, textMarkdown,
                  title: context.tr("home.map_data_source_copyright_title"),
                  markdown: true);
            },
            child: Container(
              padding: const EdgeInsets.all(4),
              decoration: BoxDecoration(
                color: Colors.black.withValues(alpha: 0.45),
                shape: BoxShape.circle,
              ),
              child: const Icon(
                Icons.info_outline,
                size: 14,
                color: Colors.white,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
