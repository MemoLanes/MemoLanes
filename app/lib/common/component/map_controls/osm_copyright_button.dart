import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/utils.dart';

class OSMCopyrightButton extends StatelessWidget {
  const OSMCopyrightButton({super.key});

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
              showCommonDialog(
                  context, context.tr("osm_data_source.content_md"),
                  title: context.tr("osm_data_source.title"), markdown: true);
            },
            child: Container(
              padding: const EdgeInsets.all(4),
              decoration: BoxDecoration(
                color: Colors.black.withOpacity(0.45),
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
