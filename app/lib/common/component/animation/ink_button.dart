import 'package:flutter/material.dart';

class InkButton extends StatelessWidget {
  final Widget child;
  final VoidCallback? onPressed;
  final Color backgroundColor;
  final Color? overlayColor;
  final ShapeBorder shape;
  final EdgeInsetsGeometry padding;
  final double? width;
  final double? height;

  const InkButton.circle({
    super.key,
    required this.child,
    required this.onPressed,
    required this.backgroundColor,
    this.overlayColor,
    double size = 48,
  })  : shape = const CircleBorder(),
        padding = EdgeInsets.zero,
        width = size,
        height = size;

  const InkButton.pill({
    super.key,
    required this.child,
    required this.onPressed,
    required this.backgroundColor,
    this.overlayColor,
    this.padding = const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
  })  : shape = const StadiumBorder(),
        width = null,
        height = null;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: width,
      height: height,
      child: Material(
        color: backgroundColor,
        shape: shape,
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onPressed,
          customBorder: shape,
          overlayColor: overlayColor == null
              ? null
              : WidgetStatePropertyAll<Color?>(overlayColor),
          child: Padding(
            padding: padding,
            child: Center(
              widthFactor: 1,
              heightFactor: 1,
              child: child,
            ),
          ),
        ),
      ),
    );
  }
}
