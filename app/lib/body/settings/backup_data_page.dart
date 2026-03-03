import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';

// TODO: This is currently unused.
class BackupDataPage extends StatelessWidget {
  const BackupDataPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("data.backup_data.title"),
      ),
      body: MlSingleChildScrollView(
        padding: EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("data.backup_data.last_backup_time"),
            position: LabelTilePosition.top,
            trailing: LabelTileContent(
              content: '2025-07-12',
            ),
          ),
          LabelTile(
            label: context.tr("data.backup_data.backup"),
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(showArrow: true),
            onTap: () {},
          ),
          LabelTile(
            label: context.tr("data.backup_data.delete_backup_data"),
            position: LabelTilePosition.bottom,
            trailing: LabelTileContent(showArrow: true),
            onTap: () {},
          ),
        ],
      ),
    );
  }
}
