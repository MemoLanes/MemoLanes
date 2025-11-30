import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';

class MapSettingsPage extends StatefulWidget {
  const MapSettingsPage({super.key});

  @override
  State<MapSettingsPage> createState() => _MapSettingsPageState();
}

enum _BaseStyle {
  normal("https://tiles.openfreemap.org/styles/liberty"),
  satellite("mapbox://styles/mapbox/satellite-v9"),
  hybrid("mapbox://styles/mapbox/satellite-streets-v12"),
  none("none"),
  custom("custom");

  final String url;
  const _BaseStyle(this.url);

  static _BaseStyle fromUrl(String url) {
    return _BaseStyle.values.firstWhere(
      (e) => e.url == url,
      orElse: () => _BaseStyle.custom,
    );
  }
}

class _MapSettingsPageState extends State<MapSettingsPage> {
  late _BaseStyle _current;
  String _customUrl = "";
  double _fogDensity = 0.5;

  @override
  void initState() {
    super.initState();
    final style = MMKVUtil.getString(MMKVKey.mapStyle,
        defaultValue: _BaseStyle.normal.url);
    _current = _BaseStyle.fromUrl(style);
    if (_current == _BaseStyle.custom) {
      _customUrl = style;
    }
    final fogDensityStr =
        MMKVUtil.getString(MMKVKey.fogDensity, defaultValue: "0.5");
    _fogDensity = double.tryParse(fogDensityStr) ?? 0.5;
  }

  String get _currentLabel => switch (_current) {
        _BaseStyle.satellite =>
          context.tr("general.map_settings.style_satellite"),
        _BaseStyle.hybrid => context.tr("general.map_settings.style_hybrid"),
        _BaseStyle.none => context.tr("general.map_settings.style_none"),
        _BaseStyle.normal => context.tr("general.map_settings.style_normal"),
        _BaseStyle.custom => context.tr("general.map_settings.style_custom"),
      };

  void _updateStyle(_BaseStyle style, {String? customUrl}) {
    if (style == _BaseStyle.custom) {
      if (customUrl != null && customUrl.isNotEmpty) {
        setState(() {
          _current = style;
          _customUrl = customUrl;
        });
        MMKVUtil.putString(MMKVKey.mapStyle, customUrl);
      }
    } else if (_current != style) {
      setState(() => _current = style);
      MMKVUtil.putString(MMKVKey.mapStyle, style.url);
    }
  }

  void _updateFogDensity(double value) {
    setState(() {
      _fogDensity = value;
    });
    MMKVUtil.putString(MMKVKey.fogDensity, value.toString());
  }

  void _showCustomUrlDialog() {
    final controller = TextEditingController(text: _customUrl);
    showDialog(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(context.tr("general.map_settings.enter_custom_url")),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(
            hintText: "mapbox://styles/mapbox/streets-v12",
            border: OutlineInputBorder(),
          ),
          autofocus: true,
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(dialogContext),
            child: Text(context.tr("common.cancel")),
          ),
          TextButton(
            onPressed: () {
              final text = controller.text;
              if (text.startsWith("http") || text.startsWith("mapbox://")) {
                Navigator.pop(dialogContext);
                _updateStyle(_BaseStyle.custom, customUrl: text);
              } else {
                showDialog(
                  context: dialogContext,
                  builder: (warningContext) => AlertDialog(
                    title: Text(
                        context.tr("general.map_settings.url_invalid_title")),
                    content: Text(
                        context.tr("general.map_settings.url_invalid_message")),
                    actions: [
                      TextButton(
                        onPressed: () => Navigator.pop(warningContext),
                        child: Text(context.tr("common.cancel")),
                      ),
                      TextButton(
                        onPressed: () {
                          Navigator.pop(warningContext);
                          Navigator.pop(dialogContext);
                          _updateStyle(_BaseStyle.custom, customUrl: text);
                        },
                        child: Text(context.tr("common.ok")),
                      ),
                    ],
                  ),
                );
              }
            },
            child: Text(context.tr("common.ok")),
          ),
        ],
      ),
    );
  }

  void _showMapStylePicker() {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            label: context.tr("general.map_settings.style_normal"),
            position: CardLabelTilePosition.top,
            onTap: () => _updateStyle(_BaseStyle.normal),
          ),
          CardLabelTile(
            label: context.tr("general.map_settings.style_satellite"),
            position: CardLabelTilePosition.middle,
            onTap: () => _updateStyle(_BaseStyle.satellite),
          ),
          CardLabelTile(
            label: context.tr("general.map_settings.style_hybrid"),
            position: CardLabelTilePosition.middle,
            onTap: () => _updateStyle(_BaseStyle.hybrid),
          ),
          CardLabelTile(
            label: context.tr("general.map_settings.style_none"),
            position: CardLabelTilePosition.middle,
            onTap: () => _updateStyle(_BaseStyle.none),
          ),
          CardLabelTile(
            label: context.tr("general.map_settings.style_custom"),
            position: CardLabelTilePosition.bottom,
            onTap: () {
              _showCustomUrlDialog();
            },
          ),
        ],
      ),
    );
  }

  void _showFogDensityPicker() {
    showBasicCard(
      context,
      child: Padding(
        padding: const EdgeInsets.all(24.0),
        child: StatefulBuilder(
          builder: (context, setModalState) {
            return Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  "${(_fogDensity * 100).toInt()}%",
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 16),
                Slider(
                  value: _fogDensity,
                  onChanged: (value) {
                    setModalState(() {});
                    _updateFogDensity(value);
                  },
                  min: 0.0,
                  max: 1.0,
                ),
              ],
            );
          },
        ),
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
            position: LabelTilePosition.top,
            trailing: LabelTileContent(
              content: _currentLabel,
              showArrow: true,
            ),
            onTap: _showMapStylePicker,
          ),
          LabelTile(
            label: context.tr("general.map_settings.fog_density"),
            position: LabelTilePosition.bottom,
            trailing: LabelTileContent(
              content: "${(_fogDensity * 100).toInt()}%",
              showArrow: true,
            ),
            onTap: _showFogDensityPicker,
          ),
        ],
      ),
    );
  }
}
