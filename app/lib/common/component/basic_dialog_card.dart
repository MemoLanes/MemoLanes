import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/constants/style_constants.dart';

/// Shows a centered, constrained application card.
Future<T?> showBasicDialogCard<T>(
  BuildContext context, {
  required WidgetBuilder builder,
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
    builder: (dialogContext) => AppDialogCard(
      title: title,
      showHeader: showTitle && title != null,
      maxHeightFactor: maxHeightFactor ?? 0.78,
      contentPadding: contentPadding,
      backgroundColor: backgroundColor ?? StyleConstants.canvasColor,
      actions: actions,
      child: builder(dialogContext),
    ),
  );
}

Future<T?> showBasicCard<T>(
  BuildContext context, {
  required WidgetBuilder builder,
  String? title,
}) {
  return showBasicDialogCard<T>(
    context,
    title: title,
    showTitle: title != null,
    contentPadding: EdgeInsets.fromLTRB(12, title == null ? 4 : 12, 12, 12),
    builder: builder,
  );
}
