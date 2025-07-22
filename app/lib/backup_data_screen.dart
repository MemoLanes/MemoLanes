import 'package:flutter/material.dart';
import 'package:memolanes/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/component/tiles/label_tile.dart';
import 'package:memolanes/component/tiles/label_tile_content.dart';

class BackupDataScreen extends StatelessWidget {
  const BackupDataScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text('数据备份'),
      ),
      body: MlSingleChildScrollView(
        padding: EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: '上次备份时间',
            position: LabelTilePosition.top,
            trailing: LabelTileContent(
              content: '2025-07-12',
            ),
          ),
          LabelTile(
            label: '备份',
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(showArrow: true),
            onTap: () {},
          ),
          LabelTile(
            label: '删除备份数据',
            position: LabelTilePosition.bottom,
            trailing: LabelTileContent(showArrow: true),
            onTap: () {},
          ),
        ],
      ),
    );
  }
}
