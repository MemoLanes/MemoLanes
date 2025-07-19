import 'package:flutter/material.dart';

class LabelTileContent extends StatelessWidget {
  const LabelTileContent({
    super.key,
    this.content = '',
    this.contentMaxLines = 1,
    this.showArrow = false,
  });

  final String content;

  final int contentMaxLines;

  final bool showArrow;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Text(
          content,
          style: TextStyle(
            fontSize: 14.0,
            color: const Color(0x99FFFFFF),
          ),
          textAlign: TextAlign.justify,
          maxLines: contentMaxLines,
          overflow: TextOverflow.ellipsis,
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
