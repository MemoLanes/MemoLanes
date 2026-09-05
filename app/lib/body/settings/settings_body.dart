import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/advanced_settings_page.dart';
import 'package:memolanes/body/settings/map_settings_page.dart';
import 'package:memolanes/body/settings/settings_category_pages.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/utils/nav_helper.dart';

class SettingsBody extends StatelessWidget {
  const SettingsBody({super.key});

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 560 ? 2 : 1;
        const gap = 12.0;
        final tileWidth = columns == 1
            ? constraints.maxWidth
            : (constraints.maxWidth - gap) / columns;

        return MlSingleChildScrollView(
          padding: EdgeInsets.only(
            top: 24.0,
            bottom: StyleConstants.navBarSafeArea + 16.0,
          ),
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: SizedBox(
                width: double.infinity,
                child: Text(
                  context.tr('settings.title'),
                  textAlign: TextAlign.left,
                  style: AppTypography.pageTitle.copyWith(
                    color: StyleConstants.inkColor,
                  ),
                ),
              ),
            ),
            const SizedBox(height: 20),
            Wrap(
              spacing: gap,
              runSpacing: gap,
              children: [
                _categoryTile(
                  context,
                  'journey_recording',
                  const JourneyRecordingSettingsPage(),
                  Icons.route_rounded,
                  const Color(0xFFFFD75A),
                  tileWidth,
                ),
                _categoryTile(
                  context,
                  'map',
                  const MapSettingsPage(),
                  Icons.map_outlined,
                  StyleConstants.primaryGreen,
                  tileWidth,
                ),
                _categoryTile(
                  context,
                  'appearance',
                  const AppearanceSettingsPage(),
                  Icons.tune_rounded,
                  const Color(0xFF9DB7FF),
                  tileWidth,
                ),
                _categoryTile(
                  context,
                  'data',
                  const DataManagementSettingsPage(),
                  Icons.inventory_2_outlined,
                  const Color(0xFFFFA66B),
                  tileWidth,
                ),
                _categoryTile(
                  context,
                  'advanced',
                  const AdvancedSettingsPage(),
                  Icons.build_outlined,
                  const Color(0xFFC5A3FF),
                  tileWidth,
                ),
                _categoryTile(
                  context,
                  'about',
                  const AboutSettingsPage(),
                  Icons.info_outline_rounded,
                  const Color(0xFF7EE0C1),
                  tileWidth,
                ),
              ],
            ),
          ],
        );
      },
    );
  }

  Widget _categoryTile(
    BuildContext context,
    String key,
    Widget page,
    IconData icon,
    Color accent,
    double width,
  ) {
    return SizedBox(
      width: width,
      child: LabelTile(
        label: context.tr('settings.categories.$key.title'),
        labelStyle: AppTypography.cardTitle.copyWith(
          color: StyleConstants.inkColor,
        ),
        desc: context.tr('settings.categories.$key.desc'),
        descMaxLines: 1,
        descStyle: AppTypography.caption.copyWith(
          color: StyleConstants.mutedInkColor,
        ),
        minHeight: 76,
        maxHeight: 84,
        bottom: false,
        prefix: Padding(
          padding: const EdgeInsets.only(right: 12),
          child: Container(
            width: 40,
            height: 40,
            decoration: BoxDecoration(
              color: accent.withValues(alpha: 0.14),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: accent.withValues(alpha: 0.3)),
            ),
            child: Icon(icon, color: accent, size: 21),
          ),
        ),
        trailing: const LabelTileContent(showArrow: true),
        position: LabelTilePosition.single,
        onTap: () => navigatorPush(context, page: page),
      ),
    );
  }
}
