import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/constants/style_constants.dart';

/// Time dimension: year / month / day / any.
enum TimeRulerMode {
  year,
  month,
  day,
  any,
}

typedef TimeRulerSelection = (int year, int? month, int? day);

// --- Constants ---

const double kRulerExtent = 44.0;
const double kRulerUnitSpacing = 36.0;
const Duration kRulerSnapDelay = Duration(milliseconds: 50);
const EdgeInsets kRulerMargin = EdgeInsets.symmetric(horizontal: 16);

// --- Ruler data: one interface for year/month/day to avoid repeated switch ---

abstract class _RulerData {
  int get itemCount;
  int get selectedIndex;
  String labelAt(BuildContext context, int index);
  bool indexEqualsSelection(int index);
  void reportSelection(int index);
  void notifyDisplay(int index);
}

class _YearRulerData extends _RulerData {
  _YearRulerData(
      this.earliest, this.selectedYear, this.onSelected, this.onDisplay);
  final DateTime earliest;
  final int selectedYear;
  final void Function(int year) onSelected;
  final void Function(TimeRulerSelection)? onDisplay;

  static const int _half = 30;
  int get _start =>
      (selectedYear - _half).clamp(earliest.year, DateTime.now().year);
  int get _end =>
      (selectedYear + _half).clamp(earliest.year, DateTime.now().year);

  @override
  int get itemCount => _end - _start + 1;

  @override
  int get selectedIndex => (selectedYear - _start).clamp(0, itemCount - 1);

  @override
  String labelAt(BuildContext context, int index) => '${_start + index}';

  @override
  bool indexEqualsSelection(int index) => (_start + index) == selectedYear;

  @override
  void reportSelection(int index) {
    onSelected(_start + index);
  }

  @override
  void notifyDisplay(int index) =>
      onDisplay?.call((_start + index, null, null));
}

/// Month mode: window from (earliest.year, earliest.month) to current month; no months before earliest.
class _MonthRulerData extends _RulerData {
  _MonthRulerData(this.earliest, this.selectedYear, this.selectedMonth,
      this.onSelected, this.onDisplay);
  final DateTime earliest;
  final int selectedYear;
  final int selectedMonth;
  final void Function(int y, int m) onSelected;
  final void Function(TimeRulerSelection)? onDisplay;

  static const int _half = 90;

  /// Number of months from (earliest.year, earliest.month) to (now.year, now.month) inclusive.
  int get _totalMonths {
    final now = DateTime.now();
    final n =
        (now.year - earliest.year) * 12 + (now.month - earliest.month) + 1;
    return n < 0 ? 0 : n;
  }

  /// Global index of selected month (0 = earliest month); clamped to 0 if before earliest.
  int get _centerIndex {
    final raw =
        (selectedYear - earliest.year) * 12 + (selectedMonth - earliest.month);
    final total = _totalMonths;
    if (total <= 0) return 0;
    return raw.clamp(0, total - 1);
  }

  int get _start {
    final total = _totalMonths;
    if (total <= 0) return 0;
    return (_centerIndex - _half).clamp(0, total - 1);
  }

  int get _end {
    final total = _totalMonths;
    if (total <= 0) return 0;
    return (_centerIndex + _half).clamp(0, total - 1);
  }

  /// globalIndex 0 = (earliest.year, earliest.month), then increment by month.
  (int y, int m) _at(int globalIndex) {
    final monthOffset = earliest.month - 1 + globalIndex;
    return (earliest.year + monthOffset ~/ 12, monthOffset % 12 + 1);
  }

  @override
  int get itemCount => _totalMonths <= 0 ? 0 : _end - _start + 1;

  @override
  int get selectedIndex =>
      (itemCount <= 0) ? 0 : (_centerIndex - _start).clamp(0, itemCount - 1);

  @override
  String labelAt(BuildContext context, int index) {
    final (_, m) = _at(_start + index);
    return DateFormat('MMM', Localizations.localeOf(context).toString())
        .format(DateTime(2000, m, 1));
  }

  @override
  bool indexEqualsSelection(int index) {
    final (y, m) = _at(_start + index);
    return y == selectedYear && m == selectedMonth;
  }

  @override
  void reportSelection(int index) {
    final (y, m) = _at(_start + index);
    onSelected(y, m);
  }

  @override
  void notifyDisplay(int index) {
    final (y, m) = _at(_start + index);
    onDisplay?.call((y, m, null));
  }
}

/// Day mode: window start is not before earliest, so selection is never before trajectory start.
class _DayRulerData extends _RulerData {
  _DayRulerData(this.earliest, this.selectedYear, this.selectedMonth,
      this.selectedDay, this.onSelected, this.onDisplay);
  final DateTime earliest;
  final int selectedYear;
  final int selectedMonth;
  final int selectedDay;
  final void Function(int y, int m, int d) onSelected;
  final void Function(TimeRulerSelection)? onDisplay;

  static const int _half = 180;
  static DateTime get _today =>
      DateTime(DateTime.now().year, DateTime.now().month, DateTime.now().day);
  DateTime get _earliestDay =>
      DateTime(earliest.year, earliest.month, earliest.day);

  DateTime get _selected {
    final lastDay = DateTime(selectedYear, selectedMonth + 1, 0).day;
    return DateTime(selectedYear, selectedMonth, selectedDay.clamp(1, lastDay));
  }

  DateTime get _windowStart {
    final sel = _selected;
    if (sel.isAfter(_today)) {
      final start = _today.subtract(const Duration(days: _half * 2));
      return start.isBefore(_earliestDay) ? _earliestDay : start;
    }
    final start = sel.subtract(const Duration(days: _half));
    return start.isBefore(_earliestDay) ? _earliestDay : start;
  }

  DateTime get _windowEnd {
    final sel = _selected;
    if (sel.isAfter(_today)) return _today;
    final end = sel.add(const Duration(days: _half));
    return end.isAfter(_today) ? _today : end;
  }

  @override
  int get itemCount {
    final days = _windowEnd.difference(_windowStart).inDays + 1;
    return days < 0 ? 0 : days;
  }

  @override
  int get selectedIndex {
    final days = _selected.difference(_windowStart).inDays;
    return days.clamp(0, itemCount > 0 ? itemCount - 1 : 0);
  }

  /// Day mode: DateTime + Duration handles cross-year, cross-month, and leap year correctly.
  DateTime _dateAt(int index) => _windowStart.add(Duration(days: index));

  @override
  String labelAt(BuildContext context, int index) =>
      _dateAt(index).day.toString().padLeft(2, '0');

  @override
  bool indexEqualsSelection(int index) {
    final d = _dateAt(index);
    return d.year == selectedYear &&
        d.month == selectedMonth &&
        d.day == selectedDay;
  }

  @override
  void reportSelection(int index) {
    final safeIdx = index.clamp(0, itemCount > 0 ? itemCount - 1 : 0);
    final d = _dateAt(safeIdx);
    onSelected(d.year, d.month, d.day);
  }

  @override
  void notifyDisplay(int index) {
    final d = _dateAt(index);
    onDisplay?.call((d.year, d.month, d.day));
  }
}

// --- Ruler UI ---

Widget _rulerContainer(Widget child) => ClipRRect(
      borderRadius: BorderRadius.circular(12),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
        child: Container(
          decoration: BoxDecoration(
            color: Colors.white.withValues(alpha: 0.15),
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
                color: Colors.white.withValues(alpha: 0.2), width: 1),
          ),
          child: child,
        ),
      ),
    );

/// Time ruler: horizontal scroll list that snaps to ticks (year / month / day).
class TimeRuler extends StatelessWidget {
  const TimeRuler({
    super.key,
    required this.rulerMode,
    required this.selectedYear,
    required this.selectedMonth,
    required this.selectedDay,
    required this.earliest,
    required this.onSelectionChanged,
    this.onDisplayChanged,
  });

  final TimeRulerMode rulerMode;
  final int selectedYear;
  final int selectedMonth;
  final int selectedDay;
  final DateTime? earliest;
  final void Function(TimeRulerSelection) onSelectionChanged;
  final void Function(TimeRulerSelection)? onDisplayChanged;

  @override
  Widget build(BuildContext context) {
    if (rulerMode == TimeRulerMode.any) return const SizedBox.shrink();
    return _InfiniteTimeRuler(
      key: ValueKey('ruler-$rulerMode'),
      rulerMode: rulerMode,
      selectedYear: selectedYear,
      selectedMonth: selectedMonth,
      selectedDay: selectedDay,
      earliest: earliest ?? DateTime(DateTime.now().year - 1, 1, 1),
      onSelectionChanged: onSelectionChanged,
      onDisplayChanged: onDisplayChanged,
    );
  }
}

class _InfiniteTimeRuler extends StatefulWidget {
  const _InfiniteTimeRuler({
    super.key,
    required this.rulerMode,
    required this.selectedYear,
    required this.selectedMonth,
    required this.selectedDay,
    required this.earliest,
    required this.onSelectionChanged,
    this.onDisplayChanged,
  });

  final TimeRulerMode rulerMode;
  final int selectedYear;
  final int selectedMonth;
  final int selectedDay;
  final DateTime earliest;
  final void Function(TimeRulerSelection) onSelectionChanged;
  final void Function(TimeRulerSelection)? onDisplayChanged;

  @override
  State<_InfiniteTimeRuler> createState() => _InfiniteTimeRulerState();
}

class _InfiniteTimeRulerState extends State<_InfiniteTimeRuler> {
  late ScrollController _scrollController;
  Timer? _snapTimer;
  bool _isScrolling = false;
  bool _isSnapping = false;
  int _lastHapticIndex = -1;
  double _viewportWidth = 0;

  _RulerData get _data => _buildData(widget);

  static _RulerData _buildData(_InfiniteTimeRuler w) {
    return switch (w.rulerMode) {
      TimeRulerMode.year => _YearRulerData(w.earliest, w.selectedYear,
          (y) => w.onSelectionChanged((y, null, null)), w.onDisplayChanged),
      TimeRulerMode.month => _MonthRulerData(
          w.earliest,
          w.selectedYear,
          w.selectedMonth,
          (y, m) => w.onSelectionChanged((y, m, null)),
          w.onDisplayChanged),
      TimeRulerMode.day => _DayRulerData(
          w.earliest,
          w.selectedYear,
          w.selectedMonth,
          w.selectedDay,
          (y, m, d) => w.onSelectionChanged((y, m, d)),
          w.onDisplayChanged),
      TimeRulerMode.any => throw StateError('any mode has no ruler'),
    };
  }

  @override
  void initState() {
    super.initState();
    _scrollController = ScrollController();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients) return;
      final d = _data;
      if (d.itemCount <= 0) {
        return;
      }
      final idx = d.selectedIndex.clamp(0, d.itemCount - 1);
      _lastHapticIndex = idx;
      _scrollController.jumpTo((idx * kRulerUnitSpacing).toDouble());
      d.reportSelection(idx);
    });
  }

  @override
  void didUpdateWidget(_InfiniteTimeRuler oldWidget) {
    super.didUpdateWidget(oldWidget);
    final selectionChanged = oldWidget.selectedYear != widget.selectedYear ||
        oldWidget.selectedMonth != widget.selectedMonth ||
        oldWidget.selectedDay != widget.selectedDay;
    if (!_isScrolling && selectionChanged && _data.itemCount > 0) {
      final idx = _data.selectedIndex.clamp(0, _data.itemCount - 1);
      _lastHapticIndex = idx;
      if (_scrollController.hasClients) {
        _scrollController.jumpTo((idx * kRulerUnitSpacing).toDouble());
      }
    }
  }

  @override
  void dispose() {
    _snapTimer?.cancel();
    _scrollController.dispose();
    super.dispose();
  }

  Future<void> _runSnapAndReport() async {
    if (!mounted || !_scrollController.hasClients) return;
    final idx = _indexAtOffset(_scrollController.offset);
    final aligned = _isAlignedToTick(idx);
    if (!aligned) {
      // Suppress haptics from scroll updates and the snap-end notification.
      _isSnapping = true;
      try {
        await _snapToIndex(idx);
      } finally {
        _isSnapping = false;
      }
      if (!mounted) return;
    }
    // Skip if _onScrollUpdate already vibrated for this tick.
    final shouldHaptic = !aligned || _lastHapticIndex != idx;
    _lastHapticIndex = idx;
    if (shouldHaptic) AppHaptics.selection();
    _data.notifyDisplay(idx);
    if (!_data.indexEqualsSelection(idx)) _data.reportSelection(idx);
  }

  int _indexAtOffset(double pixels) {
    final maxIdx = _data.itemCount > 0 ? _data.itemCount - 1 : 0;
    if (_viewportWidth <= 0) {
      return (pixels / kRulerUnitSpacing).round().clamp(0, maxIdx);
    }
    final centerContent = pixels + _viewportWidth / 2;
    final centerPadding = _viewportWidth / 2 - kRulerUnitSpacing / 2;
    final index = ((centerContent - centerPadding - kRulerUnitSpacing / 2) /
            kRulerUnitSpacing)
        .round();
    return index.clamp(0, maxIdx);
  }

  double _offsetForIndex(int index) => (index * kRulerUnitSpacing).toDouble();

  bool _isAlignedToTick(int index) {
    if (!_scrollController.hasClients) return false;
    return (_scrollController.offset - _offsetForIndex(index)).abs() < 1.0;
  }

  Future<void> _snapToIndex(int index) async {
    if (!_scrollController.hasClients) return;
    await _scrollController.animateTo(
      _offsetForIndex(index),
      duration: const Duration(milliseconds: 200),
      curve: Curves.easeOutCubic,
    );
  }

  void _onScrollUpdate(ScrollNotification n) {
    if (_viewportWidth <= 0) return;
    if (_isSnapping) return;
    final maxIdx = _data.itemCount > 0 ? _data.itemCount - 1 : 0;
    final bucket =
        (n.metrics.pixels / kRulerUnitSpacing).floor().clamp(0, maxIdx);
    if (bucket != _lastHapticIndex) {
      _lastHapticIndex = bucket;
      AppHaptics.selection();
      _data.notifyDisplay(bucket);
    }
  }

  void _onScrollEnd(ScrollNotification n) {
    _isScrolling = false;
    if (_isSnapping) return;
    _snapTimer?.cancel();
    if (!mounted || !_scrollController.hasClients) return;
    _snapTimer = Timer(kRulerSnapDelay, () async {
      await _runSnapAndReport();
      if (mounted) _snapTimer = null;
    });
  }

  @override
  Widget build(BuildContext context) {
    final count = _data.itemCount;
    if (count <= 0) {
      return SizedBox(
        height: kRulerExtent,
        child: Padding(
            padding: kRulerMargin, child: _rulerContainer(const SizedBox())),
      );
    }
    final selectedIndex = _data.selectedIndex.clamp(0, count - 1);
    return SizedBox(
      height: kRulerExtent,
      child: Padding(
        padding: kRulerMargin,
        child: _rulerContainer(
          LayoutBuilder(
            builder: (context, constraints) {
              final w = constraints.maxWidth;
              if (w > 0 && w != _viewportWidth) {
                WidgetsBinding.instance.addPostFrameCallback((_) {
                  if (mounted) setState(() => _viewportWidth = w);
                });
              }
              final centerPadding =
                  (w / 2 - kRulerUnitSpacing / 2).clamp(0.0, double.infinity);
              return Stack(
                alignment: Alignment.center,
                children: [
                  NotificationListener<ScrollNotification>(
                    onNotification: (n) {
                      if (n is ScrollStartNotification) {
                        _isScrolling = true;
                        _snapTimer?.cancel();
                        _snapTimer = null;
                      }
                      if (n is ScrollUpdateNotification) _onScrollUpdate(n);
                      if (n is ScrollEndNotification) _onScrollEnd(n);
                      return false;
                    },
                    child: ListView.builder(
                      controller: _scrollController,
                      scrollDirection: Axis.horizontal,
                      itemExtent: kRulerUnitSpacing,
                      itemCount: count,
                      padding: EdgeInsets.only(
                          left: centerPadding, right: centerPadding),
                      physics: const AlwaysScrollableScrollPhysics(),
                      itemBuilder: (context, i) => _buildTick(
                        context,
                        _data.labelAt(context, i),
                        i == selectedIndex,
                      ),
                    ),
                  ),
                  IgnorePointer(
                    child: Center(
                      child: Container(
                        width: 2,
                        height: kRulerExtent,
                        color: StyleConstants.defaultColor,
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }

  static Widget _buildTick(
      BuildContext context, String label, bool isSelected) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Container(
          width: 2,
          height: isSelected ? 10 : 6,
          color: isSelected
              ? StyleConstants.defaultColor
              : Colors.white.withValues(alpha: 0.5),
        ),
        SizedBox(height: isSelected ? 4 : 6),
        Text(label,
            style: TextStyle(
                color: Colors.white.withValues(alpha: 0.9), fontSize: 11)),
      ],
    );
  }
}
