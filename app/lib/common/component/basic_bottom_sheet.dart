import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/constants/style_constants.dart';

Future<T?> showBasicBottomSheet<T>(
  BuildContext context, {
  required Widget child,
  String? title,
  Widget? actions,
  bool showTitle = true,
  double? maxHeightFactor,
  EdgeInsetsGeometry contentPadding = EdgeInsets.zero,
  Color? barrierColor,
  Color? backgroundColor,
}) {
  return showAppDialog<T>(
    context,
    barrierColor: barrierColor,
    maxWidth: 440,
    child: AppDialogCard(
      title: title,
      showHeader: showTitle && title != null,
      maxHeightFactor: maxHeightFactor ?? 0.78,
      contentPadding: contentPadding,
      backgroundColor: backgroundColor ?? StyleConstants.canvasColor,
      actions: actions,
      child: child,
    ),
  );
}

Future<T?> showBasicCard<T>(
  BuildContext context, {
  required Widget child,
  String? title,
}) {
  return showBasicBottomSheet<T>(
    context,
    title: title,
    showTitle: title != null,
    contentPadding: EdgeInsets.fromLTRB(12, title == null ? 4 : 12, 12, 12),
    child: child,
  );
}
