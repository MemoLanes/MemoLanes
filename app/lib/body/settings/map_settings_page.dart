import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/map_base_style.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';

class MapSettingsPage extends StatefulWidget {
  const MapSettingsPage({super.key});

  @override
  State createState() => _MapSettingsPageState();
}

class _MapSettingsPageState extends State<MapSettingsPage> {
  late MapBaseStyle _current;

  @override
  void initState() {
    super.initState();
    final styleName = MMKVUtil.getString(
      MMKVKey.mapStyle,
      defaultValue: MapBaseStyle.normal.name,
    );
    _current = MapBaseStyle.fromName(styleName);
  }

  String get _currentLabel => context.tr(_current.i18nKey);

  void _updateStyle(MapBaseStyle style) {
    if (_current == style) return;
    setState(() => _current = style);
    // Persist enum name instead of URL.
    MMKVUtil.putString(MMKVKey.mapStyle, style.name);
  }

  void _showMapStylePicker() {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            label: context.tr(MapBaseStyle.normal.i18nKey),
            position: CardLabelTilePosition.top,
            onTap: () => _updateStyle(MapBaseStyle.normal),
          ),
          CardLabelTile(
            label: context.tr(MapBaseStyle.satellite.i18nKey),
            position: CardLabelTilePosition.middle,
            onTap: () => _updateStyle(MapBaseStyle.satellite),
          ),
          CardLabelTile(
            label: context.tr(MapBaseStyle.hybrid.i18nKey),
            position: CardLabelTilePosition.bottom,
            onTap: () => _updateStyle(MapBaseStyle.hybrid),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(context.tr("general.map_settings.title"))),
      body: MlSingleChildScrollView(
        padding: const EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("general.map_settings.style"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
              content: _currentLabel,
              showArrow: true,
            ),
            onTap: _showMapStylePicker,
          ),
        ],
      ),
    );
  }
}
