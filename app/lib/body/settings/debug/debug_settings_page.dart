import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/utils/nav_helper.dart';

import 'render_diagnostics.dart';
import 'tracelet_motion_debug_page.dart';

class DebugSettingsPage extends StatelessWidget {
  const DebugSettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final showMotion =
        kDebugMode && !kIsWeb && defaultTargetPlatform == TargetPlatform.iOS;
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('general.advanced_settings.debug.title'),
      ),
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            title: context.tr('settings.groups.diagnostics'),
            children: [
              SettingsTile(
                label: context.tr(
                  'general.advanced_settings.render_diagnostics',
                ),
                position: showMotion
                    ? LabelTilePosition.top
                    : LabelTilePosition.single,
                bottom: showMotion,
                trailing: const LabelTileContent(showArrow: true),
                onTap: () =>
                    navigatorPush(context, page: const RenderDiagnosticsPage()),
              ),
              if (showMotion)
                SettingsTile(
                  label: context.tr(
                    'general.advanced_settings.debug.motion.title',
                  ),
                  position: LabelTilePosition.bottom,
                  bottom: false,
                  trailing: const LabelTileContent(showArrow: true),
                  onTap: () => navigatorPush(
                    context,
                    page: const TraceletMotionDebugPage(),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
