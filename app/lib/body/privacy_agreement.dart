import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/widgets.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';

const int _latestVersion = 1;

Future<void> _showPrivacyDialogMethod(BuildContext context) async {
  final result = await showCommonDialog(
      context, context.tr("privacy.privacy_tip_message_md"),
      title: context.tr("privacy.privacy_tip_title"),
      confirmButtonText: context.tr("privacy.agree"),
      hasCancel: true,
      cancelButtonText: context.tr("privacy.disagree_and_exit"),
      markdown: true);

  if (result == true) {
    MMKVUtil.putInt(MMKVKey.privacyAgreementAccepted, _latestVersion);
  } else {
    exit(1);
  }
}

void showPrivacyAgreementIfNeeded(BuildContext context) {
  var acceptedVersion =
      MMKVUtil.getInt(MMKVKey.privacyAgreementAccepted, defaultValue: 0);
  if (acceptedVersion < _latestVersion) {
    _showPrivacyDialogMethod(context);
  }
}
