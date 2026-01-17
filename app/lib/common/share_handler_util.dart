import 'dart:async';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/common/log.dart';
import 'package:share_handler/share_handler.dart';

class ShareHandlerUtil {
  ShareHandlerUtil._();

  static StreamSubscription<SharedMedia> init(BuildContext context) {
    final handler = ShareHandlerPlatform.instance;

    handler.getInitialSharedMedia().then((media) {
      final attachments = media?.attachments ?? const [];
      _handleSharedFile(context, attachments);
    }).catchError((e) {
      log.error('Failed to get initial shared media: $e');
    });

    final subscription = handler.sharedMediaStream.listen((media) {
      final attachments = media.attachments ?? const [];
      _handleSharedFile(context, attachments);
    }, onError: (err) {
      log.error('Error in sharedMediaStream: $err');
    });

    return subscription;
  }

  static Future<void> _handleSharedFile(
      BuildContext context, List<SharedAttachment?> attachments) async {
    final paths = attachments
        .whereType<SharedAttachment>()
        .map((e) => e.path)
        .whereType<String>()
        .where((p) => p.isNotEmpty)
        .toList();

    if (paths.isEmpty) return;

    if (paths.length > 1) {
      await showCommonDialog(
        context,
        context.tr("import.shared_file.multi_message"),
      );
      return;
    }

    final path = paths.single;

    final lowerPath = path.toLowerCase();

    final fileName = path.split('/').last;
    final confirm = await showCommonDialog(
      context,
      context.tr("import.shared_file.confirm_message", args: [fileName]),
      hasCancel: true,
      title: context.tr("import.shared_file.confirm_title"),
    );

    if (confirm != true) return;

    if (lowerPath.endsWith('.mldx')) {
      await importMldx(context, path);
      return;
    }

    final importType = _resolveImportType(lowerPath);
    if (importType == null) {
      await showCommonDialog(
          context, context.tr("import.unsupported_file_failed"));
      return;
    }

    if (!context.mounted) return;

    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => ImportDataPage(
          path: path,
          importType: importType,
        ),
      ),
    );
  }

  static ImportType? _resolveImportType(String lowerPath) {
    const trackExtensions = ['.kml', '.gpx'];
    const fowExtensions = ['.fwss', '.zip'];

    if (trackExtensions.any(lowerPath.endsWith)) {
      return ImportType.gpxOrKml;
    }

    if (fowExtensions.any(lowerPath.endsWith)) {
      return ImportType.fow;
    }

    return null;
  }
}
