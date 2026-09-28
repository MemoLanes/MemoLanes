import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/settings/import_data_page.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/common_dialog.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:path/path.dart' as p;
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:zikzak_share_handler/zikzak_share_handler.dart';

enum _ShareState { idle, confirming, importing }

/// Confirms the latest share after UI startup and imports one file at a time.
class ShareHandlerUtil {
  static const _mldxExtension = '.mldx';
  static const _importTypes = {
    '.kml': ImportType.vector,
    '.gpx': ImportType.vector,
    '.csv': ImportType.vector,
    '.fwss': ImportType.fow,
    '.zip': ImportType.fow,
  };

  ShareHandlerUtil({
    required this.navigatorKey,
    required this.uiReady,
    ShareHandlerPlatform? handler,
  }) : _handler = handler ?? ShareHandlerPlatform.instance;

  final GlobalKey<NavigatorState> navigatorKey;
  final Future<void> uiReady;
  final ShareHandlerPlatform _handler;
  StreamSubscription<SharedMedia>? _subscription;
  final _pendingPaths = ValueNotifier<List<String>>([]);
  _ShareState _state = _ShareState.idle;
  OverlayEntry? _busyNotice;
  bool _receivedStreamShare = false;
  bool _disposed = false;

  void init() {
    if (_disposed || _subscription != null) return;
    // Subscribe before reading the initial share, so events arriving during
    // that asynchronous read are also retained.
    _subscription = _handler.sharedMediaStream.listen(
      _receive,
      onError: (err) {
        log.error('Error in sharedMediaStream: $err');
      },
    );
    unawaited(_readInitialShare());
  }

  Future<void> _readInitialShare() async {
    try {
      final media = await _handler.getInitialSharedMedia();
      if (media != null && !_disposed) {
        _receive(media, isInitial: true);
        await _handler.resetInitialSharedMedia();
      }
    } catch (e, s) {
      log.error('Failed to get initial shared media: $e', s);
    }
  }

  void _receive(SharedMedia media, {bool isInitial = false}) {
    if (_disposed || (isInitial && _receivedStreamShare)) return;
    final paths = (media.attachments ?? const <SharedAttachment>[])
        .map((e) => e.path)
        // Check for blank input without trimming actual file names.
        .where((path) => path.trim().isNotEmpty)
        .toList();

    if (paths.isEmpty) return;
    if (!isInitial) _receivedStreamShare = true;

    if (_state == _ShareState.importing) {
      _showBusyNotice();
      return;
    }

    _pendingPaths.value = paths;
    if (_state != _ShareState.idle) return;
    _state = _ShareState.confirming;
    unawaited(_process());
  }

  void _showBusyNotice() {
    if (_disposed || _busyNotice != null) return;
    final overlay = navigatorKey.currentState?.overlay;
    if (overlay == null || !overlay.mounted) return;

    // An overlay does not become a route, so import pages can still replace
    // or close their own routes while this notice is visible.
    final notice = OverlayEntry(
      builder: (context) => Stack(
        fit: StackFit.expand,
        children: [
          ModalBarrier(
            dismissible: false,
            color: StyleConstants.shadowColor.withValues(
              alpha: StyleConstants.isDarkMode ? 0.58 : 0.22,
            ),
          ),
          Dialog(
            elevation: 0,
            backgroundColor: Colors.transparent,
            insetPadding: const EdgeInsets.symmetric(
              horizontal: 24,
              vertical: 24,
            ),
            child: PointerInterceptor(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: SizedBox(
                  width: double.infinity,
                  child: CommonDialog(
                    title: context.tr('import.shared_file.busy_title'),
                    content: context.tr('import.shared_file.busy_message'),
                    buttons: [
                      DialogButton(
                        text: context.tr('common.ok'),
                        onPressed: _dismissBusyNotice,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
    overlay.insert(notice);
    _busyNotice = notice;
  }

  void _dismissBusyNotice() {
    final notice = _busyNotice;
    _busyNotice = null;
    notice?.remove();
    notice?.dispose();
  }

  Future<void> _process() async {
    try {
      await uiReady;
      if (_disposed) return;
      final context = navigatorKey.currentState?.context;
      if (context == null || !context.mounted) {
        throw StateError('Share import requires a mounted Navigator');
      }
      final path = await _confirmShare(context);
      if (path == null || _disposed || !context.mounted) return;
      await _handleSharedFile(context, path);
    } catch (e, s) {
      log.error('Failed to handle shared file: $e', s);
    } finally {
      _dismissBusyNotice();
      _state = _ShareState.idle;
    }
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _dismissBusyNotice();
    await _subscription?.cancel();
    _pendingPaths.dispose();
  }

  Future<String?> _confirmShare(BuildContext context) {
    return showAppDialog<String>(
      context,
      barrierDismissible: false,
      builder: (dialogContext) => ValueListenableBuilder<List<String>>(
        valueListenable: _pendingPaths,
        builder: (context, paths, _) {
          final singleFile = paths.length == 1;
          return CommonDialog(
            title: context.tr(
              singleFile ? 'import.shared_file.confirm_title' : 'common.info',
            ),
            content: singleFile
                ? context.tr(
                    'import.shared_file.confirm_message',
                    args: [p.basename(paths.single)],
                  )
                : context.tr('import.shared_file.multi_message'),
            buttons: [
              if (singleFile)
                DialogButton(
                  text: context.tr('common.cancel'),
                  variant: AppButtonVariant.secondary,
                  onPressed: () => popCurrentRoute(dialogContext),
                ),
              DialogButton(
                text: context.tr('common.ok'),
                onPressed: () {
                  // Read the current value even if a new share arrived before
                  // the dialog had a chance to rebuild. Lock before closing.
                  final latest = _pendingPaths.value;
                  if (_disposed || latest.length != 1) {
                    popCurrentRoute(dialogContext);
                    return;
                  }
                  if (ModalRoute.of(dialogContext)?.isCurrent != true) return;
                  _state = _ShareState.importing;
                  popCurrentRoute(dialogContext, latest.single);
                },
              ),
            ],
          );
        },
      ),
    );
  }

  Future<void> _handleSharedFile(BuildContext context, String path) async {
    if (p.extension(path).toLowerCase() == _mldxExtension) {
      await importMldx(context, path);
      return;
    }

    final importType = resolveImportType(path);
    if (importType == null) {
      await showCommonDialog(
        context,
        context.tr("import.unsupported_file_failed"),
      );
      return;
    }

    final flow = ImportFlow();
    await flow.waitFor(
      navigatorPush<void>(
        context,
        page: ImportDataPage(
          path: path,
          importType: importType,
          onImportContinued: flow.continueWith,
        ),
      ),
    );
  }

  static ImportType? resolveImportType(String path) {
    return _importTypes[p.extension(path).toLowerCase()];
  }
}
