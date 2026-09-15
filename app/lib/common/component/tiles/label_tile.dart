import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

enum LabelTilePosition { single, top, middle, bottom }

class LabelTile extends StatelessWidget {
  const LabelTile({
    super.key,
    this.position = LabelTilePosition.single,
    required this.label,
    this.desc = '',
    this.descMaxLines = 1,
    this.prefix,
    this.suffix,
    this.trailing,
    this.mainAxisAlignment = MainAxisAlignment.start,
    this.onTap,
    this.infoLabelOnTap,
    this.onLongPress,
    this.decoration,
    this.bottom = true,
    this.maxHeight,
    this.minHeight = 54.0,
  });

  final LabelTilePosition position;

  final String label;

  final String desc;

  final int descMaxLines;

  final Widget? prefix;

  final Widget? suffix;

  final Widget? trailing;

  final MainAxisAlignment mainAxisAlignment;

  final Function()? onTap;

  final Function()? infoLabelOnTap;

  final Function()? onLongPress;

  final BoxDecoration? decoration;

  final bool bottom;

  final double? maxHeight;

  final double minHeight;

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
      borderRadius = borderRadius.copyWith(topLeft: radius, topRight: radius);
    }

    List<Widget> children = [
      Flexible(
        child: GestureDetector(
          onTap: infoLabelOnTap,
          child: Row(
            children: [
              Flexible(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      label,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: AppTypography.itemTitle.copyWith(
                        color: StyleConstants.inkColor,
                      ),
                    ),
                    if (desc.isNotEmpty)
                      Text(
                        desc,
                        maxLines: descMaxLines,
                        overflow: TextOverflow.ellipsis,
                        style: AppTypography.caption.copyWith(
                          color: StyleConstants.mutedInkColor,
                        ),
                      ),
                  ],
                ),
              ),
              if (infoLabelOnTap != null) ...[
                const SizedBox(width: 6),
                Icon(
                  Icons.info_outline,
                  size: 18.0,
                  color: StyleConstants.mutedInkColor,
                ),
              ],
            ],
          ),
        ),
      ),
    ];
    if (prefix != null) children.insert(0, prefix!);
    if (suffix != null) children.add(suffix!);
    if (trailing != null) {
      children.add(trailing!);
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
                  maxHeight: maxHeight ?? double.infinity,
                  minHeight: minHeight,
                ),
                child: Ink(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 16,
                    vertical: 8,
                  ),
                  decoration: BoxDecoration(
                    color: StyleConstants.surfaceColor,
                    borderRadius: borderRadius,
                    border: Border(
                      bottom:
                          position == LabelTilePosition.top ||
                              position == LabelTilePosition.middle
                          ? BorderSide(color: StyleConstants.lineColor)
                          : BorderSide.none,
                    ),
                  ),
                  child: IntrinsicHeight(
                    child: Row(
                      mainAxisAlignment: mainAxisAlignment,
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: children,
                    ),
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
