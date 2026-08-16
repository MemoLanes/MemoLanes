import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;

Future<void> showAutoSelectedPreprocessorNotice(
  BuildContext context, {
  required import_api.ImportPreprocessor preprocessor,
}) async {
  switch (preprocessor) {
    case import_api.ImportPreprocessor.spare:
      await showCommonDialog(
        context,
        context.tr('import.preprocessor.spare_md'),
        markdown: true,
      );
      return;
    case import_api.ImportPreprocessor.none:
    case import_api.ImportPreprocessor.generic:
    case import_api.ImportPreprocessor.flightTrack:
      return;
  }
}
