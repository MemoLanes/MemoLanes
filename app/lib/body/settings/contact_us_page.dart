import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:url_launcher/url_launcher_string.dart';

class ContactUsPage extends StatefulWidget {
  const ContactUsPage({super.key});

  @override
  State<ContactUsPage> createState() => _ContactUsPage();
}

class _ContactUsPage extends State<ContactUsPage> {
  @override
  Widget build(BuildContext context) {
    const websiteUrl = "https://app.memolanes.com";
    const rednoteUrl =
        "https://www.xiaohongshu.com/user/profile/65cdef57000000000401c526";
    const qqGroupText = "755295072";
    const emailText = "support@memolanes.com";
    return Scaffold(
      appBar: AppBar(
        title: Text(context.tr("contact_us.title")),
      ),
      body: MlSingleChildScrollView(
        padding: EdgeInsets.all(8.0),
        children: [
          LabelTile(
            label: context.tr("contact_us.website"),
            position: LabelTilePosition.top,
            trailing: LabelTileContent(rightIcon: Icons.open_in_new),
            onTap: () async {
              await launchUrlString(websiteUrl,
                  mode: LaunchMode.externalApplication);
            },
          ),
          LabelTile(
            label: context.tr("contact_us.rednote"),
            position: LabelTilePosition.middle,
            trailing: LabelTileContent(rightIcon: Icons.open_in_new),
            onTap: () async {
              await launchUrlString(rednoteUrl,
                  mode: LaunchMode.externalApplication);
            },
          ),
          LabelTile(
            label: context.tr("contact_us.qq_group"),
            position: LabelTilePosition.middle,
            trailing:
                LabelTileContent(content: qqGroupText, rightIcon: Icons.copy),
            onTap: () {
              Clipboard.setData(ClipboardData(text: qqGroupText));
              Fluttertoast.showToast(msg: context.tr("common.copy_success"));
            },
          ),
          LabelTile(
            label: context.tr("contact_us.email"),
            position: LabelTilePosition.bottom,
            trailing:
                LabelTileContent(content: emailText, rightIcon: Icons.copy),
            onTap: () {
              Clipboard.setData(ClipboardData(text: emailText));
              Fluttertoast.showToast(msg: context.tr("common.copy_success"));
            },
          )
        ],
      ),
    );
  }
}
