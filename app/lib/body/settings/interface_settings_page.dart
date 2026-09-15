import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:provider/provider.dart';

class InterfaceSettingsPage extends StatelessWidget {
  const InterfaceSettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final controller = context.watch<AppThemeController>();

    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('general.interface_settings.title'),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8),
        children: [
          AppOptionTile(
            icon: Icons.brightness_auto_outlined,
            title: context.tr('general.interface_settings.mode_name.system'),
            selected: controller.preference == AppThemePreference.system,
            trailing: AppOptionTileTrailing.selection,
            onTap: () => controller.setPreference(AppThemePreference.system),
          ),
          const SizedBox(height: 8),
          AppOptionTile(
            icon: Icons.light_mode_outlined,
            title: context.tr('general.interface_settings.mode_name.light'),
            selected: controller.preference == AppThemePreference.light,
            trailing: AppOptionTileTrailing.selection,
            onTap: () => controller.setPreference(AppThemePreference.light),
          ),
          const SizedBox(height: 8),
          AppOptionTile(
            icon: Icons.dark_mode_outlined,
            title: context.tr('general.interface_settings.mode_name.dark'),
            selected: controller.preference == AppThemePreference.dark,
            trailing: AppOptionTileTrailing.selection,
            onTap: () => controller.setPreference(AppThemePreference.dark),
          ),
        ],
      ),
    );
  }
}
