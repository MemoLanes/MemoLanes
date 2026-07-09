import 'package:flutter/foundation.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/achievement/layer.dart';
import 'package:memolanes/src/rust/achievement/read_model/region.dart';
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

@immutable
class AchievementCountryStats {
  const AchievementCountryStats({
    required this.entityId,
    required this.isoCode,
    required this.nameKey,
    required this.visitedKm2,
    required this.totalKm2,
  });

  final achievement_api.GeoEntityId entityId;
  final String isoCode;
  final String nameKey;
  final double visitedKm2;
  final double totalKm2;

  double get progress => AchievementAreaStats._safeShare(visitedKm2, totalKm2);

  bool approximatelyEquals(
    AchievementCountryStats other, {
    double epsilonKm2 = 0.000001,
  }) {
    return entityId == other.entityId &&
        isoCode == other.isoCode &&
        nameKey == other.nameKey &&
        (visitedKm2 - other.visitedKm2).abs() < epsilonKm2 &&
        (totalKm2 - other.totalKm2).abs() < epsilonKm2;
  }

  @override
  bool operator ==(Object other) {
    return other is AchievementCountryStats &&
        entityId == other.entityId &&
        isoCode == other.isoCode &&
        nameKey == other.nameKey &&
        visitedKm2 == other.visitedKm2 &&
        totalKm2 == other.totalKm2;
  }

  @override
  int get hashCode =>
      Object.hash(entityId, isoCode, nameKey, visitedKm2, totalKm2);
}

class AchievementStatsStore extends ChangeNotifier {
  AchievementAreaStats? _stats;
  List<AchievementCountryStats> _countries = const [];
  bool _hasCountryStats = false;
  Object? _areaStatsError;
  Object? _countryStatsError;
  Future<void>? _areaInFlight;
  Future<void>? _countryStatsInFlight;

  AchievementAreaStats? get stats => _stats;
  List<AchievementCountryStats> get countries => _countries;
  Object? get areaStatsError => _areaStatsError;
  Object? get countryStatsError => _countryStatsError;
  bool get isLoading => isAreaStatsLoading || isCountryStatsLoading;
  bool get isAreaStatsLoading => _areaInFlight != null;
  bool get isCountryStatsLoading => _countryStatsInFlight != null;
  bool get hasStats => _stats != null;
  bool get hasCountryStats => _hasCountryStats;

  Future<void> refresh() async {
    await Future.wait<void>([
      _refreshAreaStats(),
      _refreshCountryStats(),
    ]);
  }

  Future<void> _refreshAreaStats() {
    final inFlight = _areaInFlight;
    if (inFlight != null) return inFlight;

    _areaStatsError = null;
    final future = _loadAndUpdateAreaStats();
    _areaInFlight = future;
    notifyListeners();
    return future;
  }

  Future<void> _refreshCountryStats() {
    final inFlight = _countryStatsInFlight;
    if (inFlight != null) return inFlight;

    _countryStatsError = null;
    final future = _loadAndUpdateCountryStats();
    _countryStatsInFlight = future;
    notifyListeners();
    return future;
  }

  Future<void> _loadAndUpdateAreaStats() async {
    var didChange = false;

    try {
      final nextStats = await _fetchAreaStats();
      final currentStats = _stats;
      if (currentStats == null ||
          !currentStats.approximatelyEquals(nextStats)) {
        _stats = nextStats;
        didChange = true;
      }
    } catch (error, stackTrace) {
      log.error('load achievement stats failed: $error', stackTrace);
      _areaStatsError = error;
      didChange = true;
    }

    _areaInFlight = null;
    if (didChange) notifyListeners();
  }

  Future<void> _loadAndUpdateCountryStats() async {
    var didChange = false;

    try {
      final nextCountries = await _fetchCountryStats();
      if (!_countryListsApproximatelyEqual(_countries, nextCountries)) {
        _countries = List.unmodifiable(nextCountries);
        _hasCountryStats = true;
        didChange = true;
      } else if (!_hasCountryStats) {
        _hasCountryStats = true;
        didChange = true;
      }
    } catch (error, stackTrace) {
      log.error('load achievement countries failed: $error', stackTrace);
      _countryStatsError = error;
      didChange = true;
    }

    _countryStatsInFlight = null;
    if (didChange) notifyListeners();
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

  Future<List<AchievementCountryStats>> _fetchCountryStats() async {
    // TODO: Move this aggregation into the Rust read-model when it exposes a
    // visited-country summary API.
    final rootCountriesView = await achievement_api.regionLevelView(
      layer: AchievementLayer.default_,
      level: RegionKind.country,
    );

    final continentsView = await achievement_api.regionLevelView(
      layer: AchievementLayer.default_,
      level: RegionKind.continent,
    );

    final countryViews = await Future.wait(
      continentsView.entries.keys.map(
        (continentId) => achievement_api.regionLevelView(
          layer: AchievementLayer.default_,
          level: RegionKind.country,
          parent: continentId,
        ),
      ),
    );

    final countryEntriesById = <achievement_api.GeoEntityId, RegionEntity>{};
    countryEntriesById.addAll(rootCountriesView.entries);
    for (final view in countryViews) {
      countryEntriesById.addAll(view.entries);
    }

    final countries = countryEntriesById.entries
        .where((entry) => entry.value.visitedAreaM2 > BigInt.zero)
        .map(
          (entry) => AchievementCountryStats(
            entityId: entry.key,
            isoCode: entry.value.isoCode,
            nameKey: entry.value.nameKey,
            visitedKm2: entry.value.visitedAreaM2.toDouble() / 1000000,
            totalKm2: entry.value.totalAreaM2.toDouble() / 1000000,
          ),
        )
        .toList()
      ..sort((a, b) {
        final areaOrder = b.visitedKm2.compareTo(a.visitedKm2);
        if (areaOrder != 0) return areaOrder;
        return a.isoCode.compareTo(b.isoCode);
      });

    return countries;
  }

  static bool _countryListsApproximatelyEqual(
    List<AchievementCountryStats> a,
    List<AchievementCountryStats> b,
  ) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (!a[i].approximatelyEquals(b[i])) return false;
    }
    return true;
  }
}
