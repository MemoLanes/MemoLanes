import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/advanced_settings_page.dart';
import 'package:memolanes/body/settings/map_settings_page.dart';
import 'package:memolanes/body/settings/settings_category_pages.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/utils/nav_helper.dart';

class SettingsBody extends StatelessWidget {
  const SettingsBody({super.key});

  @override
  Widget build(BuildContext context) {
    return SettingsPageLayout(
      topPadding: 24,
      bottomPadding: StyleConstants.navBarSafeArea + 16,
      children: [
        SizedBox(
          width: double.infinity,
          child: Text(
            context.tr('settings.title'),
            textAlign: TextAlign.left,
            style: AppTypography.pageTitle.copyWith(
              color: StyleConstants.inkColor,
            ),
          ),
        ),
        const SizedBox(height: 10),
        SettingsSection(
          title: context.tr('settings.groups.usage_settings'),
          children: [
            _categoryTile(
              context,
              'journey_recording',
              const JourneyRecordingSettingsPage(),
              Icons.route_rounded,
              StyleConstants.journeyYellow,
              LabelTilePosition.top,
            ),
            _categoryTile(
              context,
              'map',
              const MapSettingsPage(),
              Icons.map_outlined,
              StyleConstants.primaryGreen,
              LabelTilePosition.middle,
            ),
            _categoryTile(
              context,
              'appearance',
              const AppearanceSettingsPage(),
              Icons.tune_rounded,
              StyleConstants.journeyYellow,
              LabelTilePosition.bottom,
            ),
          ],
        ),
        SettingsSection(
          title: context.tr('settings.groups.data_system'),
          children: [
            _categoryTile(
              context,
              'data',
              const DataManagementSettingsPage(),
              Icons.inventory_2_outlined,
              StyleConstants.primaryGreen,
              LabelTilePosition.top,
            ),
            _categoryTile(
              context,
              'advanced',
              const AdvancedSettingsPage(),
              Icons.build_outlined,
              StyleConstants.journeyYellow,
              LabelTilePosition.bottom,
            ),
          ],
        ),
        SettingsSection(
          title: context.tr('settings.about'),
          children: [
            _categoryTile(
              context,
              'about',
              const AboutSettingsPage(),
              Icons.info_outline_rounded,
              StyleConstants.primaryGreen,
              LabelTilePosition.single,
            ),
          ],
        ),
      ],
    );
  }

  Widget _categoryTile(
    BuildContext context,
    String key,
    Widget page,
    IconData icon,
    Color accent,
    LabelTilePosition position,
  ) {
    return LabelTile(
      label: context.tr('settings.categories.$key.title'),
      labelStyle: AppTypography.cardTitle.copyWith(
        color: StyleConstants.inkColor,
      ),
      desc: context.tr('settings.categories.$key.desc'),
      descMaxLines: 1,
      descStyle: AppTypography.caption.copyWith(
        color: StyleConstants.mutedInkColor,
      ),
      minHeight: 64,
      position: position,
      prefix: Padding(
        padding: const EdgeInsets.only(right: 12),
        child: Container(
          width: 34,
          height: 34,
          decoration: BoxDecoration(
            color: accent.withValues(alpha: 0.14),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(icon, color: accent, size: 19),
        ),
      ),
      trailing: const LabelTileContent(showArrow: true),
      onTap: () => navigatorPush(context, page: page),
    );
  }
}
