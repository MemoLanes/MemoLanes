import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class AchievementBody extends StatefulWidget {
  const AchievementBody({super.key});

  @override
  AchievementBodyState createState() => AchievementBodyState();
}

class AchievementBodyState extends State<AchievementBody> {
  double? _exploredAreaInSquareKM;
  Timer? _updateTimer;

  void _loadExploredArea() async {
    var exploredAreaInSquareMeter = await api.areaOfMainMap();
    setState(() {
      if (exploredAreaInSquareMeter == null) {
        _exploredAreaInSquareKM = null;
      } else {
        _exploredAreaInSquareKM =
            exploredAreaInSquareMeter.toDouble() / 1_000_000;
      }
    });
  }

  @override
  void initState() {
    super.initState();
    _loadExploredArea();
    _updateTimer = Timer.periodic(Duration(seconds: 5), (timer) {
      _loadExploredArea();
    });
  }

  @override
  void dispose() {
    _updateTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    var areaFormat = NumberFormat()
      ..minimumFractionDigits = 4
      ..maximumFractionDigits = 4;
    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      body: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 420),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 28),
              decoration: BoxDecoration(
                color:
                    colorScheme.surfaceContainerHighest.withValues(alpha: 0.45),
                borderRadius: BorderRadius.circular(20),
                border: Border.all(
                  color: colorScheme.outlineVariant.withValues(alpha: 0.5),
                ),
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        Icons.emoji_events_outlined,
                        size: 30,
                        color: colorScheme.primary,
                      ),
                      const SizedBox(width: 10),
                      Text(
                        context.tr("achievement.title"),
                        style: const TextStyle(
                          fontSize: 26,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 14),
                  Text(
                    context.tr("achievement.coming_soon"),
                    style: TextStyle(
                      color: colorScheme.primary,
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                    ),
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 18),
                  _exploredAreaInSquareKM == null
                      ? const SizedBox.shrink()
                      : Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 8,
                          ),
                          decoration: BoxDecoration(
                            color: colorScheme.surface,
                            borderRadius: BorderRadius.circular(12),
                          ),
                          child: Text(
                            context.tr(
                              "achievement.explored_area",
                              args: [
                                areaFormat.format(_exploredAreaInSquareKM)
                              ],
                            ),
                            style: TextStyle(
                              color: colorScheme.onSurfaceVariant,
                              fontSize: 14,
                            ),
                            textAlign: TextAlign.center,
                          ),
                        ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
