import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/basic_dialog_card.dart';
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/map_fog_style.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/mmkv_util.dart';

class MapSettingsPage extends StatefulWidget {
  const MapSettingsPage({super.key});

  @override
  State createState() => _MapSettingsPageState();
}

class _MapSettingsPageState extends State<MapSettingsPage> {
  late MapStyle _current;
  late MapFogStyle _currentFogStyle;

  @override
  void initState() {
    super.initState();
    final id = MMKVUtil.getStringOpt(MMKVKey.mapStyle);
    _current = MapStyle.findById(id);
    final fogModeId = MMKVUtil.getStringOpt(MMKVKey.mapFogMode);
    _currentFogStyle = MapFogStyle.findById(fogModeId);
  }

  String _labelFor(MapStyle style) {
    return context.tr("general.map_settings.style_name.${style.id}");
  }

  String _fogStyleLabelFor(MapFogStyle style) {
    return context.tr("general.map_settings.fog_mode_name.${style.id}");
  }

  void _updateStyle(MapStyle style) {
    if (_current.id == style.id) return;
    setState(() => _current = style);
    MMKVUtil.putString(MMKVKey.mapStyle, style.id);
  }

  void _updateFogStyle(MapFogStyle style) {
    if (_currentFogStyle.id == style.id) return;
    setState(() => _currentFogStyle = style);
    MMKVUtil.putString(MMKVKey.mapFogMode, style.id);
  }

  void _showMapStylePicker() {
    showBasicCard(
      context,
      title: context.tr("general.map_settings.style"),
      builder: (dialogContext) => Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final mapStyle in MapStyle.all) ...[
            AppOptionTile(
              icon: Icons.map_outlined,
              title: _labelFor(mapStyle),
              selected: _current.id == mapStyle.id,
              trailing: AppOptionTileTrailing.selection,
              onTap: () {
                Navigator.of(dialogContext).pop();
                _updateStyle(mapStyle);
              },
            ),
            if (mapStyle != MapStyle.all.last) const SizedBox(height: 8),
          ],
        ],
      ),
    );
  }

  void _showFogStylePicker() {
    showBasicCard(
      context,
      title: context.tr("general.map_settings.fog_mode"),
      builder: (dialogContext) => Column(
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
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("general.map_settings.title"),
      ),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("general.map_settings.style"),
            position: LabelTilePosition.top,
            trailing: LabelTileContent(
              content: _labelFor(_current),
              showArrow: true,
            ),
            onTap: _showMapStylePicker,
          ),
          LabelTile(
            label: context.tr("general.map_settings.fog_mode"),
            position: LabelTilePosition.bottom,
            trailing: LabelTileContent(
              content: _fogStyleLabelFor(_currentFogStyle),
              showArrow: true,
            ),
            onTap: _showFogStylePicker,
          ),
        ],
      ),
    );
  }
}
