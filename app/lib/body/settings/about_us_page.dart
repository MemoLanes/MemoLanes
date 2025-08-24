import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/body/settings/raw_data_page.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher_string.dart';

class aboutUsPage extends StatefulWidget {
  const aboutUsPage({super.key});

  @override
  State<aboutUsPage> createState() => _AboutUsPage();
}

class _AboutUsPage extends State<aboutUsPage> {
  @override
  Widget build(BuildContext context) {
    var gpsManager = context.watch<GpsManager>();

    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("about-us.title")),
      ),
      body: MlSingleChildScrollView(
        padding: EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("about-us.website.name"),
            position: LabelTilePosition.top,
            trailing: LabelTileContent(rightIcon: Icons.open_in_new),
            onTap: () async {
              await launchUrlString(context.tr("about-us.website.url"),
                  mode: LaunchMode.externalApplication);
            },
          ),
          LabelTile(
            label: context.tr("about-us.red-note.name"),
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(
                content: context.tr("about-us.red-note.content"),
                rightIcon: Icons.copy),
            onTap: () {
              Clipboard.setData(
                  ClipboardData(text: context.tr("about-us.red-note.content")));
              Fluttertoast.showToast(msg: context.tr("common.copy_success"));
            },
          ),
          LabelTile(
            label: context.tr("about-us.qq.name"),
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(
                content: context.tr("about-us.qq.content"),
                rightIcon: Icons.copy),
            onTap: () {
              Clipboard.setData(
                  ClipboardData(text: context.tr("about-us.qq.content")));
              Fluttertoast.showToast(msg: context.tr("common.copy_success"));
            },
          ),
          LabelTile(
            label: context.tr("about-us.email.name"),
            position: LabelTilePosition.bottom,
            trailing: LabelTileContent(
                content: context.tr("about-us.email.content"),
                rightIcon: Icons.copy),
            onTap: () {
              Clipboard.setData(
                  ClipboardData(text: context.tr("about-us.email.content")));
              Fluttertoast.showToast(msg: context.tr("common.copy_success"));
            },
          )
        ],
      ),
    );
  }
}
