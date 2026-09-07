import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/import_loading_page.dart';
import 'package:memolanes/body/settings/mldx_import_page.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/common/log.dart';

bool popCurrentRoute<T>(BuildContext context, [T? result]) {
  if (!context.mounted) return false;

  final route = ModalRoute.of(context);
  if (route?.isCurrent != true) return false;

  final navigator = Navigator.of(context);
  if (!navigator.canPop()) return false;

  navigator.pop<T>(result);
  return true;
}

Future<bool> showCommonDialog(
  BuildContext context,
  String message, {
  bool hasCancel = false,
  String? title,
  String? confirmButtonText,
  String? cancelButtonText,
  AppButtonVariant confirmVariant = AppButtonVariant.primary,
  bool markdown = false,
}) async {
  final resolvedConfirmButtonText =
      confirmButtonText ?? context.tr("common.ok");
  final resolvedCancelButtonText =
      cancelButtonText ?? context.tr("common.cancel");
  final dialogTitle = title ?? context.tr("common.info");
  final result = await showAppDialog<bool>(
    context,
    barrierDismissible: false,
    builder: (dialogContext) => CommonDialog(
      title: dialogTitle,
      content: message,
      buttons: [
        if (hasCancel)
          DialogButton(
            text: resolvedCancelButtonText,
            variant: AppButtonVariant.secondary,
            onPressed: () => Navigator.of(dialogContext).pop(false),
          ),
        DialogButton(
          text: resolvedConfirmButtonText,
          variant: confirmVariant,
          onPressed: () => Navigator.of(dialogContext).pop(true),
        ),
      ],
      markdown: markdown,
    ),
  );
  return result ?? false;
}

Future<T> showLoadingDialog<T>({required Future<T> asyncTask}) async {
  final result = await GlobalLoadingManager.instance.runWithLoading<T>(
    () => asyncTask,
  );
  return result;
}

Future<void> importMldx(BuildContext context, String path) async {
  await Navigator.of(context).push<void>(
    MaterialPageRoute(
      builder: (context) => ImportLoadingPage(
        filePath: path,
        load: () async {
          final mldxFile = await OpaqueMldxReader.open(mldxFilePath: path);
          final preview = await mldxFile.analyze();
          return (mldxFile, preview);
        },
        onLoaded: (loadingContext, result) async {
          final (mldxFile, preview) = result;
          final unchangedCount = preview
              .where((j) => j.$2 == MldxJourneyImportAnalyzeResult.unchanged)
              .length;
          final importableCount = preview
              .where((j) => j.$2 != MldxJourneyImportAnalyzeResult.unchanged)
              .length;

          if (importableCount == 0 && unchangedCount > 0) {
            await showCommonDialog(
              loadingContext,
              loadingContext.tr(
                'import.mldx_preview.all_skipped',
                args: ['$unchangedCount'],
              ),
            );
            if (loadingContext.mounted) {
              popCurrentRoute(loadingContext);
            }
          } else if (loadingContext.mounted) {
            await Navigator.of(loadingContext).pushReplacement<bool, void>(
              MaterialPageRoute(
                builder: (context) =>
                    MldxImportPage(journeys: preview, mldxReader: mldxFile),
              ),
            );
          }
        },
        onError: (loadingContext, error, stackTrace) async {
          log.error('[import_mldx] Data parsing failed $error', stackTrace);
          await showCommonDialog(
            loadingContext,
            loadingContext.tr('import.parsing_failed'),
          );
          if (loadingContext.mounted) {
            popCurrentRoute(loadingContext);
          }
        },
      ),
    ),
  );
}
