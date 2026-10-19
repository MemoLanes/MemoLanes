import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';

class SettingsTile extends StatelessWidget {
  const SettingsTile({
    super.key,
    required this.label,
    this.labelStyle,
    this.labelTrailing,
    this.position = LabelTilePosition.single,
    this.bottom = true,
    this.prefix,
    this.trailing,
    this.onTap,
  });

  final String label;
  final TextStyle? labelStyle;
  final Widget? labelTrailing;
  final LabelTilePosition position;
  final bool bottom;
  final Widget? prefix;
  final Widget? trailing;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return LabelTile(
      label: label,
      labelStyle: labelStyle,
      labelTrailing: labelTrailing,
      position: position,
      bottom: bottom,
      prefix: prefix,
      trailing: trailing,
      onTap: onTap,
      minHeight: 56,
      // Leave room for a 48px switch target and the bottom border within 56px.
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 3),
    );
  }
}

// Keep the existing settings API while sharing its content frame.
typedef SettingsPageFrame = PageContentFrame;
typedef SettingsPageLayout = PageContentLayout;

class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.children,
    this.titleColor,
  });

  final String title;
  final List<Widget> children;
  final Color? titleColor;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 12, 8, 6),
            child: Text(
              title,
              style: AppTypography.sectionLabel.copyWith(
                color: titleColor ?? context.appColors.deepGreen,
              ),
            ),
          ),
          OptionCard(useSafeArea: false, separators: false, children: children),
          const SizedBox(height: 4),
        ],
      ),
    );
  }
}
