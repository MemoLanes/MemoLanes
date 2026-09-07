import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';

class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.children,
  });

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 12, 8, 6),
            child: Text(
              title,
              style: AppTypography.sectionLabel.copyWith(
                color: const Color(0x99FFFFFF),
              ),
            ),
          ),
          ...children,
          const SizedBox(height: 4),
        ],
      ),
    );
  }
}
