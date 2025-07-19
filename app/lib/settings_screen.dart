import 'package:flutter/material.dart';

import 'component/scroll_views/single_child_scroll_view.dart';
import 'component/tiles/label_tile.dart';
import 'component/tiles/label_tile_content.dart';
import 'component/tiles/label_tile_title.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return MlSingleChildScrollView(
      padding: EdgeInsets.symmetric(vertical: 16.0),
      children: [
        CircleAvatar(
          backgroundColor: const Color(0xFFB6E13D),
          radius: 45.0,
        ),
        Padding(
          padding: EdgeInsets.symmetric(vertical: 16.0),
          child: Text(
            'Ryan Schnetzer',
            style: TextStyle(
              fontSize: 24.0,
              color: const Color(0xFFFFFFFF),
            ),
          ),
        ),
        LabelTileTitle(
          label: '通用',
        ),
        LabelTile(
          label: '版本信息',
          position: LabelTilePosition.middle,
          trailing: Row(
            children: [
              LabelTileContent(
                content: 'v1.5.3',
                showArrow: false,
              ),
              Padding(
                padding: EdgeInsets.only(left: 4.0),
                child: Container(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.all(Radius.circular(12.0)),
                    border: Border.all(
                      color: const Color(0xFFFF0000),
                      width: 1,
                    ),
                  ),
                  child: Padding(
                    padding: EdgeInsets.all(2.0),
                    child: Text(
                      '有新版',
                      style: TextStyle(
                        color: const Color(0xFFFF0000),
                        fontSize: 10.0,
                      ),
                    ),
                  ),
                ),
              )
            ],
          ),
          onTap: () {},
        ),
        LabelTile(
          label: '高级设置',
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTileTitle(
          label: '数据',
        ),
        LabelTile(
          label: '数据备份',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: '数据导入',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: '数据导出',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: '清除 App 数据',
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTileTitle(
          label: '关于我们',
        ),
        LabelTile(
          label: '个人隐私政策',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: 'App 开源项目使用',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: '联系开发者',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: 'FAQ',
          position: LabelTilePosition.middle,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        LabelTile(
          label: '建议',
          position: LabelTilePosition.bottom,
          trailing: LabelTileContent(),
          onTap: () {},
        ),
        SizedBox(height: 96.0),
      ],
    );
  }
}

class UpdateNotifier extends ChangeNotifier {
  String? updateUrl;

  void setUpdateUrl(String? url) {
    updateUrl = url;
    notifyListeners();
  }

  bool hasUpdateNotification() {
    return updateUrl != null;
  }
}
