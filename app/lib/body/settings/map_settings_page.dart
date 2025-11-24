import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';

/// 仅实现页面 UI，不做任何设置存取或全局状态改动。
/// 包含一个“底图样式”项，点击后弹出底部选择框：地图 / 卫星图。
class MapSettingsPage extends StatefulWidget {
  const MapSettingsPage({super.key});

  @override
  State<MapSettingsPage> createState() => _MapSettingsPageState();
}

enum _BaseStyle { normal, satellite, hybrid }

class _MapSettingsPageState extends State<MapSettingsPage> {
  _BaseStyle _current = _BaseStyle.normal; // 仅用于本页展示，不写入设置

  String get _currentLabel => _current == _BaseStyle.satellite
      ? context.tr("general.map_settings.style_satellite")
      : _current == _BaseStyle.hybrid
          ? context.tr("general.map_settings.style_hybrid")
          : context.tr("general.map_settings.style_normal");

  void _showPicker() {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            label: context.tr("general.map_settings.style_normal"),
            position: CardLabelTilePosition.top,
            onTap: () {
              if (_current != _BaseStyle.normal) {
                setState(() => _current = _BaseStyle.normal);
              }
            },
          ),
          CardLabelTile(
            label: context.tr("general.map_settings.style_satellite"),
            position: CardLabelTilePosition.middle,
            onTap: () {
              if (_current != _BaseStyle.satellite) {
                setState(() => _current = _BaseStyle.satellite);
              }
            },
          ),
          CardLabelTile(
            label: context.tr("general.map_settings.style_hybrid"),
            position: CardLabelTilePosition.bottom,
            onTap: () {
              if (_current != _BaseStyle.hybrid) {
                setState(() => _current = _BaseStyle.hybrid);
              }
            },
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
            onTap: _showPicker,
          ),
        ],
      ),
    );
  }
}
