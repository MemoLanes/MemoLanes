import 'dart:ui' show lerpDouble;

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/theme/app_colors.dart';

void showMapCopyrightDialog(BuildContext context, {MapStyleStore? store}) {
  final styles = store ?? MapStyleStore.instance;
  final current = styles.selected;
  final entries = styles.copyrightEntries(current);
  final name = current.customName?.replaceAllMapped(
    RegExp(r'[\\`*_{}\[\]<>!#|]'),
    (match) => '\\${match[0]}',
  );
  final content = [if (name != null) '**$name**', ...entries].join('\n\n');
  showCommonDialog(
    context,
    content,
    title: context.tr('home.map_data_source_copyright_title'),
    markdown: true,
  );
}

class MapCopyrightButton extends StatelessWidget {
  static const double iconSize = 14;
  static const double contentPadding = 4;
  static const double buttonOpacity = 0.70;
  static const double buttonSize = iconSize + contentPadding * 2;

  const MapCopyrightButton({super.key});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => showMapCopyrightDialog(context),
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
