import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:provider/provider.dart';

Future<void> showAppearancePicker(BuildContext context) {
  final controller = context.read<AppThemeController>();

  return showBasicCard<void>(
    context,
    title: context.tr('general.appearance.title'),
    builder: (dialogContext) => Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final preference in AppThemePreference.values) ...[
          AppOptionTile(
            icon: switch (preference) {
              AppThemePreference.system => Icons.brightness_auto_outlined,
              AppThemePreference.light => Icons.light_mode_outlined,
              AppThemePreference.dark => Icons.dark_mode_outlined,
            },
            title: dialogContext.tr(
              'general.appearance.mode_name.${preference.id}',
            ),
            selected: controller.preference == preference,
            trailing: AppOptionTileTrailing.selection,
            onTap: () {
              Navigator.of(dialogContext).pop();
              controller.setPreference(preference);
            },
          ),
          if (preference != AppThemePreference.values.last)
            const SizedBox(height: 8),
        ],
      ],
    ),
  );
}
