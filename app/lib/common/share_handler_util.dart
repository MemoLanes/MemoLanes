import 'dart:async';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/common/log.dart';
import 'package:share_handler/share_handler.dart';

class ShareHandlerUtil {
  ShareHandlerUtil._();

  static GlobalKey<NavigatorState>? _navigatorKey;
  static List<SharedAttachment?>? _pendingShare;

  /// Subscribes to share intents immediately. [navigatorKey] is used to obtain
  /// context for dialogs/navigation when handling shares.
  static StreamSubscription<SharedMedia> init({
    GlobalKey<NavigatorState>? navigatorKey,
  }) {
    _navigatorKey = navigatorKey;
    final handler = ShareHandlerPlatform.instance;

    handler.getInitialSharedMedia().then((media) {
      final attachments = media?.attachments ?? const [];
      _setPendingAndProcess(attachments);
    }).catchError((e) {
      log.error('Failed to get initial shared media: $e');
    });

    final subscription = handler.sharedMediaStream.listen((media) {
      final attachments = media.attachments ?? const [];
      _setPendingAndProcess(attachments);
    }, onError: (err) {
      log.error('Error in sharedMediaStream: $err');
    });

    return subscription;
  }

  static void _setPendingAndProcess(List<SharedAttachment?> attachments) {
    if (attachments.isEmpty) return;
    _pendingShare = attachments;
    WidgetsBinding.instance.addPostFrameCallback((_) => _processPending());
  }

  static Future<void> _processPending() async {
    final ctx = _navigatorKey?.currentState?.context;
    if (ctx == null || !ctx.mounted) return;
    final attachments = _pendingShare;
    _pendingShare = null;
    if (attachments == null) return;
    await _handleSharedFile(ctx, attachments);
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
