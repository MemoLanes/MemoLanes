import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/storage.dart';

class RawDataSwitch extends StatefulWidget {
  const RawDataSwitch({super.key});

  @override
  State<RawDataSwitch> createState() => _RawDataSwitchState();
}

class _RawDataSwitchState extends State<RawDataSwitch> {
  bool enabled = false;

  @override
  initState() {
    super.initState();
    api.getRawDataMode().then((value) => setState(() {
          enabled = value;
        }));
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.symmetric(horizontal: 8.0),
      child: LabelTile(
        label: context.tr("general.advance_settings.raw_data_mode"),
        position: LabelTilePosition.single,
        trailing: Switch(
          value: enabled,
          onChanged: (bool value) async {
            await api.toggleRawDataMode(enable: value);
            setState(() {
              enabled = value;
            });
          },
        ),
      ),
    );
  }
}

class RawDataPage extends StatefulWidget {
  const RawDataPage({super.key});

  @override
  State<RawDataPage> createState() => _RawDataPage();
}

class _RawDataPage extends State<RawDataPage> {
  List<RawDataFile> items = [];

  @override
  void initState() {
    super.initState();
    _loadList();
  }

  void _loadList() async {
    var list = await api.listAllRawData();
    setState(() {
      items = list;
    });
  }

  void _showExportCard(BuildContext context, String filePath) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("general.advance_settings.raw_data_export_csv"),
            onTap: () {
              showCommonExport(context, filePath, deleteFile: false);
            },
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("general.advance_settings.raw_data_export_gpx"),
            onTap: () async {
              final gpxPath =
                  await api.exportRawDataGpxFile(csvFilepath: filePath);
              showCommonExport(context, gpxPath, deleteFile: true);
            },
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("general.advance_settings.raw_data_mode")),
      ),
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const SizedBox(height: 8),
          const RawDataSwitch(),
          const SizedBox(height: 16),
          Expanded(
            child: ListView(
              shrinkWrap: true,
              children: items.map((item) {
                return ListTile(
                  leading: const Icon(Icons.description),
                  title: Text(item.name),
                  onTap: () {
                    _showExportCard(context, item.path);
                  },
                  trailing: ElevatedButton(
                    onPressed: () async {
                      if (await showCommonDialog(
                          context, context.tr("journey.delete_journey_message"),
                          hasCancel: true,
                          title: context.tr("journey.delete_journey_title"),
                          confirmButtonText: context.tr("common.delete"),
                          confirmGroundColor: Colors.red,
                          confirmTextColor: Colors.white)) {
                        await api.deleteRawDataFile(filename: item.name);
                        _loadList();
                      }
                    },
                    child: const Icon(Icons.delete),
                  ),
                );
              }).toList(),
            ),
          ),
        ],
      ),
    );
  }
}
