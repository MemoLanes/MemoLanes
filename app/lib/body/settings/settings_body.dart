import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/advanced_settings_page.dart';
import 'package:memolanes/body/settings/map_settings_page.dart';
import 'package:memolanes/body/settings/settings_category_pages.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/theme/app_colors.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:provider/provider.dart';

class SettingsBody extends StatelessWidget {
  const SettingsBody({
    super.key,
    required this.topSafeArea,
    required this.bottomSafeArea,
  });

  final double topSafeArea;
  final double bottomSafeArea;

  @override
  Widget build(BuildContext context) {
    return SettingsPageLayout(
      topPadding: topSafeArea + 24,
      bottomPadding: bottomSafeArea + 16,
      framePadding: const EdgeInsets.symmetric(horizontal: 8),
      children: [
        SizedBox(
          width: double.infinity,
          child: Text(
            context.tr('settings.title'),
            textAlign: TextAlign.left,
            style: AppTypography.pageTitle.copyWith(
              color: context.appColors.inkColor,
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
              context.appColors.deepYellow,
              LabelTilePosition.top,
            ),
            _categoryTile(
              context,
              'map',
              const MapSettingsPage(),
              Icons.map_outlined,
              context.appColors.deepGreen,
              LabelTilePosition.middle,
            ),
            _categoryTile(
              context,
              'appearance',
              const AppearanceSettingsPage(),
              Icons.palette_outlined,
              context.appColors.deepYellow,
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
              context.appColors.deepGreen,
              LabelTilePosition.top,
            ),
            _categoryTile(
              context,
              'advanced',
              const AdvancedSettingsPage(),
              Icons.build_outlined,
              context.appColors.deepYellow,
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
              context.appColors.deepGreen,
              LabelTilePosition.single,
              hasUpdate: context.watch<UpdateNotifier>().updateUrl != null,
            ),
          ],
        ),
        const _SettingsVersionFooter(),
      ],
    );
  }

  Widget _categoryTile(
    BuildContext context,
    String key,
    Widget page,
    IconData icon,
    Color accent,
    LabelTilePosition position, {
    bool hasUpdate = false,
  }) {
    return LabelTile(
      label: context.tr('settings.categories.$key.title'),
      labelStyle: AppTypography.cardTitle.copyWith(
        color: context.appColors.inkColor,
      ),
      desc: context.tr('settings.categories.$key.desc'),
      descMaxLines: 1,
      descStyle: AppTypography.caption.copyWith(
        color: context.appColors.mutedInkColor,
      ),
      minHeight: 64,
      position: position,
      bottom: false,
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
      trailing: hasUpdate
          ? Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  padding: const EdgeInsets.all(2),
                  decoration: BoxDecoration(
                    color: context.appColors.deepGreen,
                    borderRadius: BorderRadius.circular(5),
                  ),
                  child: Text(
                    'NEW',
                    style: AppTypography.badge.copyWith(
                      color: context.appColors.inverseInkColor,
                    ),
                  ),
                ),
                const SizedBox(width: 10),
                const LabelTileContent(showArrow: true),
              ],
            )
          : const LabelTileContent(showArrow: true),
      onTap: () => navigatorPush(context, page: page),
    );
  }
}

class _SettingsVersionFooter extends StatefulWidget {
  const _SettingsVersionFooter();

  @override
  State<_SettingsVersionFooter> createState() => _SettingsVersionFooterState();
}

class _SettingsVersionFooterState extends State<_SettingsVersionFooter> {
  String _version = '';

  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform().then((info) {
      if (!mounted) return;
      setState(() {
        _version =
            '${info.version} (${info.buildNumber}) [${api.shortCommitHash()}]';
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Center(
        child: Text(
          _version,
          textAlign: TextAlign.center,
          style: AppTypography.caption.copyWith(
            color: context.appColors.mutedInkColor,
          ),
        ),
      ),
    );
  }
}
