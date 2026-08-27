import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

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
        color: StyleConstants.surfaceColor,
        borderRadius: borderRadius,
      ),
      child: ConstrainedBox(
        constraints: BoxConstraints(
          minWidth: double.infinity,
          maxHeight: 54.0,
        ),
        child: Text(
          label,
          style: AppTypography.sectionLabel.copyWith(
            color: StyleConstants.deepGreen,
          ),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    );
  }
}
