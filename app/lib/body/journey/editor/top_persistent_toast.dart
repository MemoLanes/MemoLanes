import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:memolanes/common/component/app_modal_region.dart';

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

    // Update the mounted hint without removing and reinserting its overlay.
    if (_overlayEntry != null) {
      _lastMessage = message;
      _overlayEntry!.markNeedsBuild();
      return;
    }

    _lastMessage = message;

    _overlayEntry = OverlayEntry(
      builder: (overlayContext) {
        final theme = Theme.of(overlayContext);
        final dialogBg =
            theme.dialogTheme.backgroundColor ?? theme.colorScheme.surface;
        final okText = overlayContext.tr("common.ok");
        final bodyText = _lastMessage ?? '';

        return AppModalRegion.dialog(
          alignment: const Alignment(0, -0.6),
          child: PointerInterceptor(
            child: AppDialogSurface(
              backgroundColor: dialogBg.withValues(alpha: 0.75),
              borderRadius: BorderRadius.circular(14),
              child: SingleChildScrollView(
                primary: false,
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(bodyText, style: theme.textTheme.bodyMedium),
                    ),
                    TextButton(onPressed: hide, child: Text(okText)),
                  ],
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
}
