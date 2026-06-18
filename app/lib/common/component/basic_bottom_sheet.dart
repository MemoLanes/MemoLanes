import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';

class BasicBottomSheet extends StatelessWidget {
  const BasicBottomSheet({
    super.key,
    required this.child,
    this.title,
    this.actions,
    this.leading,
    this.showHandle = true,
    this.showTitle = true,
    this.maxHeightFactor,
    this.contentPadding = EdgeInsets.zero,
    this.backgroundColor = Colors.black,
  });

  final Widget child;
  final String? title;
  final Widget? actions;
  final Widget? leading;
  final bool showHandle;
  final bool showTitle;
  final double? maxHeightFactor;
  final EdgeInsetsGeometry contentPadding;
  final Color backgroundColor;

  @override
  Widget build(BuildContext context) {
    final content = Container(
      constraints: maxHeightFactor == null
          ? null
          : BoxConstraints(
              maxHeight: MediaQuery.of(context).size.height * maxHeightFactor!,
            ),
      decoration: BoxDecoration(
        color: backgroundColor,
        borderRadius: const BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 12.0),
            child: Offstage(
              offstage: !showHandle,
              child: Center(
                child: CustomPaint(
                  size: const Size(40.0, 4.0),
                  painter: LinePainter(
                    color: const Color(0xFFB5B5B5),
                  ),
                ),
              ),
            ),
          ),
          if (showTitle && title != null)
            Padding(
              padding:
                  const EdgeInsets.symmetric(horizontal: 8.0, vertical: 4.0),
              child: Row(
                children: [
                  leading ?? const SizedBox(width: 48),
                  Expanded(
                    child: Text(
                      title!,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 18,
                        fontWeight: FontWeight.w600,
                      ),
                      textAlign: TextAlign.center,
                    ),
                  ),
                  const SizedBox(width: 48),
                ],
              ),
            ),
          Flexible(
            child: SingleChildScrollView(
              padding: contentPadding,
              child: child,
            ),
          ),
          if (actions != null) actions!,
        ],
      ),
    );

    return content;
  }
}

Future<T?> showBasicBottomSheet<T>(
  BuildContext context, {
  required Widget child,
  String? title,
  Widget? actions,
  Widget? leading,
  bool showHandle = true,
  bool showTitle = true,
  double? maxHeightFactor,
  EdgeInsetsGeometry contentPadding = EdgeInsets.zero,
  Color? barrierColor,
  Color backgroundColor = Colors.black,
}) {
  return showModalBottomSheet<T>(
    context: context,
    backgroundColor: Colors.transparent,
    barrierColor: barrierColor,
    isScrollControlled: true,
    isDismissible: true,
    builder: (context) {
      return BasicBottomSheet(
        title: title,
        actions: actions,
        leading: leading,
        showHandle: showHandle,
        showTitle: showTitle,
        maxHeightFactor: maxHeightFactor,
        contentPadding: contentPadding,
        backgroundColor: backgroundColor,
        child: child,
      );
    },
  );
}

void showBasicCard(
  BuildContext context, {
  required Widget child,
  bool showHandle = true,
}) {
  showBasicBottomSheet<void>(
    context,
    showHandle: showHandle,
    showTitle: false,
    child: child,
  );
}

Future<T?> showBasicCardWithResult<T>(
  BuildContext context, {
  required String title,
  required Widget child,
  String? primaryButtonText,
  VoidCallback? onPrimaryPressed,
  bool showHandle = true,
  bool showLeading = true,
}) {
  return showBasicBottomSheet<T>(
    context,
    title: title,
    showHandle: showHandle,
    showTitle: showLeading,
    maxHeightFactor: 0.75,
    leading: IconButton(
      icon: const Icon(Icons.arrow_back_ios, color: Colors.white),
      onPressed: () => Navigator.of(context).pop(),
    ),
    actions: primaryButtonText != null && onPrimaryPressed != null
        ? Padding(
            padding: const EdgeInsets.fromLTRB(24, 16, 24, 24),
            child: SizedBox(
              width: double.infinity,
              child: FilledButton(
                onPressed: onPrimaryPressed,
                style: FilledButton.styleFrom(
                  backgroundColor: const Color(0xFF007AFF),
                  padding: const EdgeInsets.symmetric(vertical: 16),
                ),
                child: Text(primaryButtonText),
              ),
            ),
          )
        : null,
    child: child,
  );
}
