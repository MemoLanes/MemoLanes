import 'package:flutter/material.dart';

class LabelTileContent extends StatelessWidget {
  const LabelTileContent({
    super.key,
    this.content = '',
    this.contentMaxLines = 1,
    this.showArrow = false,
    this.maxWidthPercent = 0.6,
    this.rightIcon,
    this.rightIconColor,
  });

  final String content;

  final int contentMaxLines;

  final double maxWidthPercent;

  final bool showArrow;

  final IconData? rightIcon;

  final Color? rightIconColor;

  Widget _buildContent(BuildContext context) {
    final width = MediaQueryData.fromView(View.of(context)).size.width;
    return ConstrainedBox(
      constraints: BoxConstraints(
        maxWidth: width * maxWidthPercent,
      ),
      child: Text(
        content,
        style: const TextStyle(
          fontSize: 14.0,
          color: Color(0x99FFFFFF),
        ),
        textAlign: TextAlign.justify,
        maxLines: contentMaxLines,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }

  Widget? _buildIcon() {
    final IconData? effectiveIcon =
        rightIcon ?? (showArrow ? Icons.arrow_forward_ios : null);
    if (effectiveIcon == null) return null;

    return Icon(
      effectiveIcon,
      size: 16.0,
      color: rightIconColor ?? const Color(0x99FFFFFF),
    );
  }

  @override
  Widget build(BuildContext context) {
    final icon = _buildIcon();
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        _buildContent(context),
        if (icon != null) ...[
          const SizedBox(width: 8),
          icon,
        ],
      ],
    );
  }
}
