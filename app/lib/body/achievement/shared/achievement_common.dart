import 'package:flutter/material.dart';

const achievementCardPadding = EdgeInsets.all(16);

bool useCompactAchievementCardLayout(BuildContext context) {
  return MediaQuery.sizeOf(context).width < 470;
}

class AchievementProgressLine extends StatelessWidget {
  const AchievementProgressLine({
    super.key,
    required this.progress,
    required this.accent,
    this.height = 8,
  });

  final double progress;
  final Color accent;
  final double height;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(999),
      child: SizedBox(
        height: height,
        child: Stack(
          fit: StackFit.expand,
          children: [
            ColoredBox(color: Colors.white.withValues(alpha: 0.08)),
            FractionallySizedBox(
              alignment: Alignment.centerLeft,
              widthFactor: progress,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: accent,
                  borderRadius: BorderRadius.circular(999),
                  boxShadow: [
                    BoxShadow(
                      color: accent.withValues(alpha: 0.45),
                      blurRadius: 10,
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
