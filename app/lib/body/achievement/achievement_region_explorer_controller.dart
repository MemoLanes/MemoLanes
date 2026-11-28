import 'package:flutter/foundation.dart';
import 'package:memolanes/body/achievement/achievement_region_sort.dart';
import 'package:memolanes/body/achievement/shared/achievement_region_rows.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement;

class _NavigationFrame {
  const _NavigationFrame({
    required this.parent,
    required this.rows,
    required this.selectedId,
    required this.outline,
    required this.scrollOffset,
    required this.sortMode,
    required this.hasRegionStats,
  });

  final achievement.GeoEntityId? parent;
  final RegionRows rows;
  final achievement.GeoEntityId? selectedId;
  final achievement.RegionGeometry? outline;
  final double scrollOffset;
  final AchievementRegionSort sortMode;
  final bool hasRegionStats;
}

/// List navigation and accepted map state; no map renderer is owned here.
class AchievementRegionExplorerController extends ChangeNotifier {
  AchievementRegionExplorerController({
    required this.worldviewManager,
    Future<RegionRows> Function()? loadRoot,
    Future<RegionRows> Function(achievement.GeoEntityId)? loadChildren,
    this._loadGeometry,
  }) : _loadRoot = loadRoot ?? loadAchievementRegionRows,
       _loadChildren = loadChildren ?? loadAchievementRegionRows {
    _activating = worldviewManager.addActivationListener(_onActivation);
  }

  final WorldviewManager worldviewManager;
  final Future<RegionRows> Function() _loadRoot;
  final Future<RegionRows> Function(achievement.GeoEntityId) _loadChildren;
  final Future<achievement.RegionGeometry?> Function(achievement.GeoEntityId)?
  _loadGeometry;
  final List<_NavigationFrame> _frames = [];
  int _stageGeneration = 0;
  int _selectionGeneration = 0;
  int _worldviewGeneration = 0;
  late bool _activating;
  bool _disposed = false;
  bool _awaitingInitialRoot = false;
  achievement.GeoEntityId? _parent;
  achievement.RegionGeometry? _cachedGeometry;
  Future<void>? _geometryRepair;

  RegionRows rows = const {};
  bool hasRegionStats = false;
  AchievementRegionSort sortMode = AchievementRegionSort.area;
  achievement.GeoEntityId? selectedId;
  achievement.RegionGeometry? outline;
  Object? listError;
  Object? geometryError;
  bool isLoading = false;
  bool isGeometryLoading = false;
  double? restoredScrollOffset;
  int cameraRevision = 0;
  int initialCameraRevision = 0;
  bool get canGoBack => _frames.isNotEmpty;
  achievement.GeoEntityId? get parent => _parent;

  void openRoot() {
    _awaitingInitialRoot = true;
    _frames.clear();
    _parent = null;
    rows = const {};
    hasRegionStats = false;
    sortMode = AchievementRegionSort.area;
    restoredScrollOffset = null;
    _clearSelection();
    initialCameraRevision++;
    _loadStage();
    notifyListeners();
  }

  void select(achievement.GeoEntityId id) {
    // The focused parent is not a row in its own subdivisions list.
    if (_activating || (selectedId != id && !rows.containsKey(id))) return;
    _select(id);
  }

  void drillInto(achievement.GeoEntityId id, double scrollOffset) {
    if (_activating || rows[id]?.hasChildren != true) return;
    _frames.add(
      _NavigationFrame(
        parent: _parent,
        rows: rows,
        selectedId: selectedId,
        outline: outline,
        scrollOffset: scrollOffset,
        sortMode: sortMode,
        hasRegionStats: hasRegionStats,
      ),
    );
    _parent = id;
    rows = const {};
    hasRegionStats = false;
    sortMode = AchievementRegionSort.area;
    restoredScrollOffset = null;
    _select(id);
    _loadStage();
  }

  /// Returns false at the root so the caller can close the explorer.
  bool back() {
    if (_activating || _frames.isEmpty) return false;
    final frame = _frames.removeLast();
    _stageGeneration++;
    _parent = frame.parent;
    rows = frame.rows;
    hasRegionStats = frame.hasRegionStats;
    sortMode = frame.sortMode;
    listError = null;
    isLoading = false;
    restoredScrollOffset = frame.scrollOffset;
    _clearSelection();
    selectedId = frame.selectedId;
    final saved = frame.outline;
    if (selectedId == null) {
      initialCameraRevision++;
    } else if (saved != null &&
        saved.worldview == worldviewManager.currentWorldview) {
      outline = saved;
      _cachedGeometry = saved;
      cameraRevision++;
    } else {
      _readGeometry(selectedId!);
    }
    notifyListeners();
    return true;
  }

  void changeSort(AchievementRegionSort mode) {
    if (_activating || sortMode == mode) return;
    sortMode = mode;
    notifyListeners();
  }

  void _clearSelection() {
    _selectionGeneration++;
    selectedId = null;
    outline = null;
    _cachedGeometry = null;
    geometryError = null;
    isGeometryLoading = false;
  }

  void _select(achievement.GeoEntityId id) {
    if (selectedId == id && _cachedGeometry != null) {
      outline = _cachedGeometry;
      cameraRevision++;
      notifyListeners();
      return;
    }
    _clearSelection();
    selectedId = id;
    _readGeometry(id);
    notifyListeners();
  }

  void retrySelectedGeometry() {
    final id = selectedId;
    if (_activating || id == null || geometryError == null) return;
    _readGeometry(id, repair: true);
    notifyListeners();
  }

  Future<void> _repairGeometry() async {
    final pending = _geometryRepair ??= worldviewManager.repairGeoData(
      worldviewManager.currentWorldview,
    );
    try {
      await pending;
    } finally {
      if (identical(_geometryRepair, pending)) _geometryRepair = null;
    }
  }

  void _readGeometry(achievement.GeoEntityId id, {bool repair = false}) {
    final selection = ++_selectionGeneration;
    final worldview = _worldviewGeneration;
    isGeometryLoading = true;
    geometryError = null;
    bool isCurrent() =>
        !_disposed &&
        !_activating &&
        worldview == _worldviewGeneration &&
        selection == _selectionGeneration &&
        selectedId == id;
    Future.sync(() async {
      if (repair) {
        await _repairGeometry();
      } else if (_geometryRepair case final pending?) {
        await pending;
      }
      if (!isCurrent()) return null;
      return _loadGeometry?.call(id) ??
          achievement.regionGeometry(entityId: id);
    }).then(
      (result) {
        if (!isCurrent()) return;
        isGeometryLoading = false;
        if (result != null &&
            result.entityId == id &&
            result.worldview == worldviewManager.currentWorldview) {
          outline = result;
          _cachedGeometry = result;
          cameraRevision++;
        } else {
          outline = null;
        }
        notifyListeners();
      },
      onError: (Object error) {
        if (!isCurrent()) return;
        isGeometryLoading = false;
        geometryError = error;
        notifyListeners();
      },
    );
  }

  void _loadStage() {
    final stage = ++_stageGeneration;
    final worldview = _worldviewGeneration;
    listError = null;
    isLoading = !_activating;
    if (_activating) return;
    final parent = _parent;
    Future.sync(() => parent == null ? _loadRoot() : _loadChildren(parent))
        .then(
          (result) {
            if (_disposed ||
                _activating ||
                stage != _stageGeneration ||
                worldview != _worldviewGeneration) {
              return;
            }
            hasRegionStats = result.isNotEmpty;
            rows = Map.fromEntries(
              result.entries.where(
                (entry) => entry.value.visitedAreaM2 > BigInt.zero,
              ),
            );
            isLoading = false;
            if (parent == null && _awaitingInitialRoot) {
              _awaitingInitialRoot = false;
              final ranked = rows.entries.toList()
                ..sort(
                  (a, b) => compareAchievementRegionArea(
                    aVisitedKm2: a.value.visitedAreaM2.toDouble() / 1000000,
                    aSortKey: a.value.isoA3Eh ?? a.value.nameKey.value,
                    bVisitedKm2: b.value.visitedAreaM2.toDouble() / 1000000,
                    bSortKey: b.value.isoA3Eh ?? b.value.nameKey.value,
                  ),
                );
              if (ranked.isNotEmpty) {
                _select(ranked.first.key);
                return;
              }
            }
            notifyListeners();
          },
          onError: (Object error) {
            if (_disposed ||
                _activating ||
                stage != _stageGeneration ||
                worldview != _worldviewGeneration) {
              return;
            }
            isLoading = false;
            listError = error;
            notifyListeners();
          },
        );
  }

  void retryList() {
    if (_activating) return;
    _loadStage();
    notifyListeners();
  }

  void _onActivation(bool activating) {
    if (_disposed) return;
    _worldviewGeneration++;
    _stageGeneration++;
    _activating = activating;
    if (activating) {
      _frames.clear();
      _parent = null;
      rows = const {};
      hasRegionStats = false;
      sortMode = AchievementRegionSort.area;
      listError = null;
      isLoading = false;
      restoredScrollOffset = null;
      _clearSelection();
      initialCameraRevision++;
      notifyListeners();
    } else {
      openRoot();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    worldviewManager.removeActivationListener(_onActivation);
    super.dispose();
  }
}
