import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/import_data.dart';

Future<void> showAutoSelectedPreprocessorNotice(
  BuildContext context, {
  required ImportPreprocessor preprocessor,
}) async {
  switch (preprocessor) {
    case ImportPreprocessor.spare:
      await showCommonDialog(
        context,
        context.tr('import.preprocessor.spare_md'),
        markdown: true,
      );
      return;
    case ImportPreprocessor.none:
    case ImportPreprocessor.generic:
    case ImportPreprocessor.flightTrack:
      return;
  }
}
