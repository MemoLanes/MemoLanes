import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/map_fog_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/body/settings/map_styles_page.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/body/settings/settings_section.dart';
import 'package:memolanes/theme/app_colors.dart';

class MapSettingsPage extends StatefulWidget {
  const MapSettingsPage({super.key});

  @override
  State createState() => _MapSettingsPageState();
}

class _MapSettingsPageState extends State<MapSettingsPage> {
  late MapFogStyle _currentFogStyle;
  late Worldview _worldview;

  @override
  void initState() {
    super.initState();
    final fogModeId = MMKVUtil.getStringOpt(MMKVKey.mapFogMode);
    _currentFogStyle = MapFogStyle.findById(fogModeId);
    _worldview = WorldviewManager.instance.currentWorldview;
  }

  String _fogStyleLabelFor(MapFogStyle style) {
    return context.tr("general.map_settings.fog_mode_name.${style.id}");
  }

  void _updateFogStyle(MapFogStyle style) {
    if (_currentFogStyle.id == style.id) return;
    setState(() => _currentFogStyle = style);
    MMKVUtil.putString(MMKVKey.mapFogMode, style.id);
  }

  Future<void> _showMapStylePicker() async {
    await Navigator.of(context)
        .push<void>(MaterialPageRoute(builder: (_) => const MapStylesPage()));
    if (mounted) setState(() {});
  }

  void _showFogStylePicker() {
    showAppDialog(
      context,
      builder: (dialogContext) => AppDialogContent(
        title: context.tr("general.map_settings.fog_mode"),
        contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final fogStyle in MapFogStyle.all) ...[
              AppOptionTile(
                icon: fogStyle == MapFogStyle.dark
                    ? Icons.dark_mode_outlined
                    : Icons.light_mode_outlined,
                title: _fogStyleLabelFor(fogStyle),
                selected: _currentFogStyle == fogStyle,
                trailing: AppOptionTileTrailing.selection,
                onTap: () {
                  Navigator.of(dialogContext).pop();
                  _updateFogStyle(fogStyle);
                },
              ),
              if (fogStyle != MapFogStyle.all.last) const SizedBox(height: 8),
            ],
          ],
        ),
      ),
    );
  }

  Future<void> _selectWorldview() async {
    final result = await showWorldviewPicker(
      context,
      selectedWorldview: _worldview,
    );
    if (result == null || result == _worldview || !mounted) return;
    await showLoadingDialog(
      asyncTask: WorldviewManager.instance.update(result),
    );
    if (mounted) setState(() => _worldview = result);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("general.map_settings.title"),
      ),
      body: SettingsPageLayout(
        children: [
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.map_display'),
            children: [
              SettingsTile(
                label: context.tr("general.map_settings.style"),
                position: LabelTilePosition.top,
                trailing: LabelTileContent(
                  content: mapStyleLabel(
                    context,
                    MapStyleStore.instance.selected,
                  ),
                  showArrow: true,
                ),
                onTap: _showMapStylePicker,
              ),
              SettingsTile(
                label: context.tr("general.map_settings.fog_mode"),
                position: LabelTilePosition.bottom,
                bottom: false,
                trailing: LabelTileContent(
                  content: _fogStyleLabelFor(_currentFogStyle),
                  showArrow: true,
                ),
                onTap: _showFogStylePicker,
              ),
            ],
          ),
          SettingsSection(
            titleColor: context.appColors.deepGreen,
            title: context.tr('settings.groups.region'),
            children: [
              SettingsTile(
                label: context.tr("privacy.region_title"),
                position: LabelTilePosition.single,
                bottom: false,
                trailing: LabelTileContent(
                  content: regionPreferenceTitle(context, _worldview),
                  showArrow: true,
                ),
                onTap: _selectWorldview,
              ),
            ],
          ),
        ],
      ),
    );
  }
}
