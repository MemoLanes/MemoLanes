import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';

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
    final topOffset = screenHeight / 5;

    final okText = context.tr("common.ok");

    _overlayEntry = OverlayEntry(
      builder: (overlayContext) {
        return Positioned(
          top: topOffset,
          left: 16,
          right: 16,
          child: Material(
            color: Colors.transparent,
            child: Dialog(
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(14),
              ),
              insetPadding: EdgeInsets.zero,
              child: Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
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
        );
      },
    );

    final overlay = Overlay.of(context, rootOverlay: true);
    overlay.insert(_overlayEntry!);
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
            Text(
              line,
              style: style,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
        ],
      );
    }

    return Text(
      message,
      style: style,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}
