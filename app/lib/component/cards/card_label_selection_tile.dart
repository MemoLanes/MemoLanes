import 'package:flutter/material.dart';

enum CardLabelSelectionTilePosition {
  single,
  top,
  middle,
  bottom,
}

class CardLabelSelectionTile extends StatelessWidget {
  const CardLabelSelectionTile({
    super.key,
    this.position = CardLabelSelectionTilePosition.single,
    required this.label,
    this.labelFontFamily,
    required this.labelValue,
    required this.value,
    this.onTap,
  });

  final CardLabelSelectionTilePosition position;

  final String label;

  final String? labelFontFamily;

  final int labelValue;

  final int? value;

  final Function(String, int)? onTap;

  @override
  Widget build(BuildContext context) {
    final radius = Radius.circular(16.0);

    BorderRadius? borderRadius = BorderRadius.zero;

    if (position == CardLabelSelectionTilePosition.single ||
        position == CardLabelSelectionTilePosition.bottom) {
      borderRadius = borderRadius.copyWith(
        bottomLeft: radius,
        bottomRight: radius,
      );
    }
    if (position == CardLabelSelectionTilePosition.single ||
        position == CardLabelSelectionTilePosition.top) {
      borderRadius = borderRadius.copyWith(
        topLeft: radius,
        topRight: radius,
      );
    }

    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: () => onTap?.call(label, labelValue),
        borderRadius: borderRadius,
        child: Ink(
          height: 54.0,
          decoration: BoxDecoration(
            borderRadius: borderRadius,
          ),
          padding: EdgeInsets.symmetric(horizontal: 16.0),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final textWidget = Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  label,
                ),
              );
              if (value == labelValue) {
                return Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    textWidget,
                    Image(
                      image: AssetImage('assets/icons/ic_tick.webp'),
                      width: 24.0,
                      height: 24.0,
                      color: const Color(0xFFB6E13D),
                    ),
                  ],
                );
              }
              return textWidget;
            },
          ),
        ),
      ),
    );
  }
}
