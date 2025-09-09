import 'package:flutter/material.dart';

enum LabelTilePosition {
  single,
  top,
  middle,
  bottom,
}

class LabelTile extends StatelessWidget {
  const LabelTile({
    super.key,
    this.position = LabelTilePosition.single,
    required this.label,
    this.desc = '',
    this.prefix,
    this.suffix,
    this.trailing,
    this.mainAxisAlignment = MainAxisAlignment.start,
    this.onTap,
    this.labelOnTap,
    this.onLongPress,
    this.decoration,
    this.bottom = true,
    this.maxHeight,
  });

  final LabelTilePosition position;

  final String label;

  final String desc;

  final Widget? prefix;

  final Widget? suffix;

  final Widget? trailing;

  final MainAxisAlignment mainAxisAlignment;

  final Function()? onTap;

  final Function()? labelOnTap;

  final Function()? onLongPress;

  final BoxDecoration? decoration;

  final bool bottom;

  final double? maxHeight;

  @override
  Widget build(BuildContext context) {
    final radius = Radius.circular(16.0);

    EdgeInsets? margin;
    BorderRadius? borderRadius = BorderRadius.zero;

    if (position == LabelTilePosition.single ||
        position == LabelTilePosition.bottom) {
      margin = EdgeInsets.only(bottom: bottom ? 8.0 : 4.0);
      borderRadius = borderRadius.copyWith(
        bottomLeft: radius,
        bottomRight: radius,
      );
    }
    if (position == LabelTilePosition.single ||
        position == LabelTilePosition.top) {
      borderRadius = borderRadius.copyWith(
        topLeft: radius,
        topRight: radius,
      );
    }

    List<Widget> children = [
      GestureDetector(
        onTap: labelOnTap,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxHeight: 54.0),
          child: Row(
            children: [
              Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  if (desc.isNotEmpty)
                    Text(
                      desc,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                ],
              ),
              if (labelOnTap != null) ...[
                const SizedBox(width: 6),
                const Icon(
                  Icons.info_outline,
                  size: 18.0,
                  color: Color(0x99FFFFFF),
                ),
              ],
            ],
          ),
        ),
      )
    ];
    if (prefix != null) children.insert(0, prefix!);
    if (suffix != null) children.add(suffix!);
    if (trailing != null) {
      children.addAll([Expanded(child: SizedBox.shrink()), trailing!]);
    }

    return Container(
      margin: margin,
      decoration: decoration,
      child: Column(
        children: [
          Material(
            color: Colors.transparent,
            child: InkWell(
              onTap: onTap,
              onLongPress: onLongPress,
              borderRadius: borderRadius,
              child: ConstrainedBox(
                constraints: BoxConstraints(
                  maxHeight: maxHeight ?? 54.0,
                  minHeight: 54.0,
                ),
                child: Ink(
                  padding: EdgeInsets.symmetric(horizontal: 16.0),
                  decoration: BoxDecoration(
                    color: const Color(0x1AFFFFFF),
                    borderRadius: borderRadius,
                  ),
                  child: Row(
                    mainAxisAlignment: mainAxisAlignment,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: children,
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
