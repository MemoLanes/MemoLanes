import 'package:flutter/material.dart';

class LabelTileTitle extends StatelessWidget {
  const LabelTileTitle({
    super.key,
    required this.label,
  });

  final String label;

  @override
  Widget build(BuildContext context) {
    final radius = Radius.circular(16.0);

    BorderRadius? borderRadius = BorderRadius.zero;
    borderRadius = borderRadius.copyWith(
      topLeft: radius,
      topRight: radius,
    );

    return Container(
      padding: EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 8.0),
      decoration: BoxDecoration(
        color: const Color(0x1AFFFFFF),
        borderRadius: borderRadius,
      ),
      child: ConstrainedBox(
        constraints: BoxConstraints(
          minWidth: double.infinity,
          maxHeight: 54.0,
        ),
        child: Text(
          label,
          style: TextStyle(color: const Color(0x99FFFFFF)),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    );
  }
}
