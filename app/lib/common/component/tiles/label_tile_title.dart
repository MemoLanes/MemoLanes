import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

class LabelTileTitle extends StatelessWidget {
  const LabelTileTitle({super.key, required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 8.0),
      child: SizedBox(
        width: double.infinity,
        child: Text(
          label,
          style: AppTypography.sectionLabel.copyWith(
            color: StyleConstants.deepGreen,
          ),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    );
  }
}
