import 'package:flutter/material.dart';

class LabelTileContent extends StatelessWidget {
  const LabelTileContent({
    super.key,
    this.content = '',
    this.contentMaxLines = 1,
    this.showArrow = false,
    this.maxWidthPercent = 0.6,
  });

  final String content;

  final int contentMaxLines;

  final double maxWidthPercent;

  final bool showArrow;

  @override
  Widget build(BuildContext context) {
    final width = MediaQueryData.fromView(View.of(context)).size.width;
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: width * maxWidthPercent,
          ),
          child: Text(
            content,
            style: TextStyle(
              fontSize: 14.0,
              color: const Color(0x99FFFFFF),
            ),
            textAlign: TextAlign.justify,
            maxLines: contentMaxLines,
            overflow: TextOverflow.ellipsis,
          ),
        ),
        showArrow
            ? Image.asset(
                'assets/icons/ic_next.webp',
                width: 16.0,
                height: 16.0,
              )
            : const SizedBox.shrink(),
      ],
    );
  }
}
