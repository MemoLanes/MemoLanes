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

    hide();
    _lastMessage = message;

    final mediaQuery = MediaQuery.of(context);
    final screenHeight = mediaQuery.size.height;
    final topOffset = screenHeight / 6;

    final okText = context.tr("common.ok");

    final theme = Theme.of(context);
    final dialogBg =
        theme.dialogTheme.backgroundColor ?? theme.colorScheme.surface;

    _overlayEntry = OverlayEntry(
      builder: (_) {
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
                          message,
                          style: Theme.of(context).textTheme.bodyMedium,
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
    try {
      _overlayEntry?.remove();
    } catch (_) {
    } finally {
      _overlayEntry = null;
      _lastMessage = null;
    }
  }

  Widget _toastText(
    String message, {
    TextStyle? style,
    bool allowExplicitNewlines = false,
  }) {
    if (allowExplicitNewlines && message.contains('\n')) {
      final lines = message.split('\n');

      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final line in lines)
            FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.centerLeft,
              child: Text(
                line,
                style: style,
                maxLines: 1,
                softWrap: false,
                overflow: TextOverflow.ellipsis,
              ),
            ),
        ],
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
