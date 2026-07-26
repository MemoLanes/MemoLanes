import 'package:flutter/material.dart';

enum CardLabelTilePosition {
  single,
  top,
  middle,
  bottom,
}

class CardLabelTile extends StatelessWidget {
  const CardLabelTile({
    super.key,
    this.alignment = Alignment.center,
    this.position = CardLabelTilePosition.single,
    required this.label,
    this.onTap,
    this.color,
    this.icon,
    this.top = true,
  });

  final AlignmentGeometry alignment;

  final CardLabelTilePosition position;

  final String label;

  final Function()? onTap;

  final Color? color;

  final IconData? icon;

  final bool top;

  @override
  Widget build(BuildContext context) {
    final radius = Radius.circular(16.0);

    EdgeInsets? margin;
    BorderRadius? borderRadius = BorderRadius.zero;

    if (position == CardLabelTilePosition.single ||
        position == CardLabelTilePosition.bottom) {
      borderRadius = borderRadius.copyWith(
        bottomLeft: radius,
        bottomRight: radius,
      );
    }
    if (position == CardLabelTilePosition.single ||
        position == CardLabelTilePosition.top) {
      margin = EdgeInsets.only(top: top ? 8.0 : 0.0);
      borderRadius = borderRadius.copyWith(
        topLeft: radius,
        topRight: radius,
      );
    }

    return Container(
      margin: margin,
      child: Column(
        children: [
          Material(
            color: Colors.transparent,
            child: InkWell(
              onTap: () {
                Navigator.pop(context);
                onTap?.call();
              },
              borderRadius: borderRadius,
              child: Ink(
                height: 54.0,
                decoration: BoxDecoration(
                  borderRadius: borderRadius,
                ),
                padding: EdgeInsets.symmetric(horizontal: 16.0),
                child: Align(
                  alignment: alignment,
                  child: icon == null
                      ? Text(
                          label,
                          style: color != null ? TextStyle(color: color) : null,
                        )
                      : Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(
                              icon,
                              size: 18.0,
                              color: color ??
                                  DefaultTextStyle.of(context).style.color,
                            ),
                            const SizedBox(width: 8.0),
                            Text(
                              label,
                              style: color != null
                                  ? TextStyle(color: color)
                                  : null,
                            ),
                          ],
                        ),
                ),
              ),
            ),
          ),
          (position == CardLabelTilePosition.top ||
                  position == CardLabelTilePosition.middle)
              ? Container(
                  height: 0.5,
                  color: const Color(0xFF262626),
                )
              : SizedBox.shrink(),
        ],
      ),
    );
  }
}
