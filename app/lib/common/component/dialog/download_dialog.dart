import 'dart:io';

import 'package:dio/dio.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/dialog/common_dialog.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';

final Dio globalDio = Dio(
  BaseOptions(
    connectTimeout: const Duration(seconds: 10),
    receiveTimeout: const Duration(seconds: 20),
  ),
);

class DownloadDialog extends StatefulWidget {
  final String url;
  final String filePath;

  const DownloadDialog({
    Key? key,
    required this.url,
    required this.filePath,
  }) : super(key: key);

  @override
  State<DownloadDialog> createState() => _DownloadDialogState();
}

class _DownloadDialogState extends State<DownloadDialog> {
  double progress = 0.0;
  bool isDownloading = false;
  CancelToken? _cancelToken;

  int _lastProgressTs = 0;

  @override
  void initState() {
    super.initState();
    _startDownload();
  }

  void safeSetState(VoidCallback fn) {
    if (!mounted) return;
    setState(fn);
  }

  Future<void> _startDownload() async {
    if (isDownloading) return;

    safeSetState(() => isDownloading = true);
    _cancelToken = CancelToken();

    final hasValidCache = await _checkLocalCache();
    if (hasValidCache) {
      safeSetState(() => isDownloading = false);
      Navigator.of(context).pop(true);
      return;
    }

    await _performDownload();

    safeSetState(() => isDownloading = false);
  }

  Future<bool> _checkLocalCache() async {
    final file = File(widget.filePath);

    try {
      final head = await globalDio.head(widget.url);
      final remoteSize =
          int.tryParse(head.headers.value('content-length') ?? "");

      if (file.existsSync() && remoteSize != null) {
        final localSize = await file.length();
        if (localSize == remoteSize) {
          safeSetState(() => progress = 1.0);
          return true;
        } else {
          await file.delete();
        }
      }
    } catch (_) {
      if (file.existsSync()) await file.delete();
    }

    return false;
  }

  Future<void> _performDownload() async {
    try {
      await globalDio.download(
        widget.url,
        widget.filePath,
        cancelToken: _cancelToken,
        deleteOnError: true,
        onReceiveProgress: (received, total) {
          if (total <= 0) return;

          final now = DateTime.now().millisecondsSinceEpoch;
          if (now - _lastProgressTs < 100) return;

          _lastProgressTs = now;
          safeSetState(() => progress = received / total);
        },
      );

      if (!mounted) return;
      Navigator.of(context).pop(true);
    } catch (e) {
      if (!mounted) return;

      if (e is DioException && e.type == DioExceptionType.cancel) {
        Navigator.of(context).pop(false);
        return;
      }

      await showCommonDialog(
        context,
        context.tr("download.error"),
      );
      log.error("[download_dialog.dart] Download failed $e");
    }
  }

  void _cancelDownload() {
    if (_cancelToken != null && !_cancelToken!.isCancelled) {
      _cancelToken!.cancel();
    }
  }

  @override
  void dispose() {
    _cancelDownload();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return CommonDialog(
      title: context.tr("download.title"),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          LinearProgressIndicator(value: progress),
          const SizedBox(height: 12),
          Text("${(progress * 100).toStringAsFixed(0)}%"),
        ],
      ),
      buttons: [
        DialogButton(
          text: context.tr("common.cancel"),
          onPressed: () {
            _cancelDownload();
            Navigator.of(context).pop(false);
          },
        ),
      ],
    );
  }
}
