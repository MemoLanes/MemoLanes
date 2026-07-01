import 'package:flutter/foundation.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/achievement/layer.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement_api;

@immutable
class AchievementAreaStats {
  const AchievementAreaStats({
    required this.totalKm2,
    required this.groundKm2,
    required this.flightKm2,
  });

  final double totalKm2;
  final double groundKm2;
  final double flightKm2;

  double get groundShare => _safeShare(groundKm2, totalKm2);
  double get flightShare => _safeShare(flightKm2, totalKm2);

  bool approximatelyEquals(
    AchievementAreaStats other, {
    double epsilonKm2 = 0.000001,
  }) {
    return (totalKm2 - other.totalKm2).abs() < epsilonKm2 &&
        (groundKm2 - other.groundKm2).abs() < epsilonKm2 &&
        (flightKm2 - other.flightKm2).abs() < epsilonKm2;
  }

  static double _safeShare(double value, double total) {
    if (total <= 0) return 0;
    return (value / total).clamp(0, 1).toDouble();
  }

  @override
  bool operator ==(Object other) {
    return other is AchievementAreaStats &&
        totalKm2 == other.totalKm2 &&
        groundKm2 == other.groundKm2 &&
        flightKm2 == other.flightKm2;
  }

  @override
  int get hashCode => Object.hash(totalKm2, groundKm2, flightKm2);
}

class AchievementStatsStore extends ChangeNotifier {
  AchievementAreaStats? _stats;
  Future<void>? _inFlight;

  AchievementAreaStats? get stats => _stats;
  bool get isLoading => _inFlight != null;
  bool get hasStats => _stats != null;

  Future<void> refresh() {
    final inFlight = _inFlight;
    if (inFlight != null) return inFlight;

    _stats = null;
    final future = _loadAndUpdate();
    _inFlight = future;
    notifyListeners();
    return future;
  }

  Future<void> _loadAndUpdate() async {
    AchievementAreaStats? nextStats;
    var didChange = false;

    try {
      nextStats = await _fetchAreaStats();
    } catch (error, stackTrace) {
      log.error('load achievement stats failed: $error', stackTrace);
    } finally {
      final currentStats = _stats;
      if (nextStats != null &&
          (currentStats == null ||
              !currentStats.approximatelyEquals(nextStats))) {
        _stats = nextStats;
        didChange = true;
      }
      _inFlight = null;
      if (didChange || currentStats == null) notifyListeners();
    }
  }

  Future<AchievementAreaStats> _fetchAreaStats() async {
    final areasByLayer = await achievement_api.getExploredAreaByLayer();
    double km2For(AchievementLayer layer) {
      return (areasByLayer[layer]?.toDouble() ?? 0) / 1000000;
    }

    return AchievementAreaStats(
      totalKm2: km2For(AchievementLayer.all),
      groundKm2: km2For(AchievementLayer.default_),
      flightKm2: km2For(AchievementLayer.flight),
    );
  }
}
