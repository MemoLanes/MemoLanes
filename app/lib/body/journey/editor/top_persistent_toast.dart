import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class TopPersistentToast {
  TopPersistentToast._internal();

  static final TopPersistentToast _instance = TopPersistentToast._internal();
  factory TopPersistentToast() => _instance;

  OverlayEntry? _overlayEntry;
  String? _lastMessage;

  void show(BuildContext context, String message) {
    if (_overlayEntry != null && _lastMessage == message) {
      return;
    }

    // Reuse the same OverlayEntry when only the text changes. The old
    // implementation always called hide() then insert(): hide() deferred
    // remove() to the next frame, so briefly two overlays existed → flicker.
    if (_overlayEntry != null) {
      _lastMessage = message;
      _overlayEntry!.markNeedsBuild();
      return;
    }

    _lastMessage = message;

    _overlayEntry = OverlayEntry(
      builder: (overlayContext) {
        final mediaQuery = MediaQuery.of(overlayContext);
        final screenHeight = mediaQuery.size.height;
        final topOffset = screenHeight / 6;

        final theme = Theme.of(overlayContext);
        final dialogBg =
            theme.dialogTheme.backgroundColor ?? theme.colorScheme.surface;
        final okText = overlayContext.tr("common.ok");
        final bodyText = _lastMessage ?? '';

        return Positioned(
          top: topOffset,
          left: 16,
          right: 16,
          child: Material(
            color: Colors.transparent,
            child: PointerInterceptor(
              child: Dialog(
                backgroundColor: dialogBg.withValues(alpha: 0.75),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(14),
                ),
                insetPadding: EdgeInsets.zero,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      Expanded(
                        child: _toastText(
                          bodyText,
                          style: theme.textTheme.bodyMedium,
                          allowExplicitNewlines: true,
                        ),
                      ),
                      TextButton(
                        onPressed: hide,
                        child: Text(okText),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );

    Overlay.of(context, rootOverlay: true).insert(_overlayEntry!);
  }

  void hide() {
    final entry = _overlayEntry;
    _overlayEntry = null;
    _lastMessage = null;

    if (entry != null) {
      entry.remove();
      entry.dispose();
    }
  }

  Widget _toastText(
    String message, {
    TextStyle? style,
    bool allowExplicitNewlines = false,
  }) {
    // Do not wrap each line in its own FittedBox: lines would scale independently
    // and look like different font sizes when widths differ.
    if (allowExplicitNewlines && message.contains('\n')) {
      return Text(
        message,
        style: style,
        textAlign: TextAlign.start,
        softWrap: true,
      );
    }

    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerLeft,
      child: Text(
        message,
        style: style,
        maxLines: 1,
        softWrap: false,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}
