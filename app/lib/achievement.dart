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
      _exploredAreaInSquareKM =
          exploredAreaInSquareMeter.toDouble() / 1_000_000;
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

    return Scaffold(
        body: Center(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const Text(
            "Achievement",
            style: TextStyle(fontSize: 24),
          ),
          const SizedBox(height: 32),
          _exploredAreaInSquareKM == null
              ? SizedBox.shrink()
              : Text(
                  "Explored Area: ${areaFormat.format(_exploredAreaInSquareKM)} km²",
                ),
        ],
      ),
    ));
  }
}
