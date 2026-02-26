import 'dart:async';
import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/body/time_machine/advance_ruler_slider.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// Time dimension: year / month / day / any; tap the button to open a single-select menu.
enum TimeMachineMode {
  year,
  month,
  day,
  any,
}

/// Time range picker: ball + year/month/day ruler or any date-range overlay.
/// Reports the selected [from]-[to] range to the parent via [onRangeChanged].
class TimeRangePicker extends StatefulWidget {
  final DateTime? earliestDate;
  final bool loading;
  final void Function(DateTime from, DateTime to) onRangeChanged;

  const TimeRangePicker({
    super.key,
    this.earliestDate,
    this.loading = false,
    required this.onRangeChanged,
  });

  @override
  State<TimeRangePicker> createState() => _TimeRangePickerState();
}

class _TimeRangePickerState extends State<TimeRangePicker> {
  TimeMachineMode _mode = TimeMachineMode.year;
  int _selectedYear = DateTime.now().year;
  int _selectedMonth = DateTime.now().month;
  int _selectedDay = DateTime.now().day;
  /// Only for button display; updates in real time while scrolling; syncs with selected on release.
  int _displayYear = DateTime.now().year;
  int _displayMonth = DateTime.now().month;
  int _displayDay = DateTime.now().day;
  DateTime _fromDate = DateTime.now();
  DateTime _toDate = DateTime.now();

  void _applyCurrentRange() {
    DateTime from;
    DateTime to;
    switch (_mode) {
      case TimeMachineMode.year:
        from = DateTime(_selectedYear, 1, 1);
        to = DateTime(_selectedYear, 12, 31);
        break;
      case TimeMachineMode.month:
        from = DateTime(_selectedYear, _selectedMonth, 1);
        to = DateTime(_selectedYear, _selectedMonth + 1, 0);
        break;
      case TimeMachineMode.day:
        final lastDay = DateTime(_selectedYear, _selectedMonth + 1, 0).day;
        final d = _selectedDay.clamp(1, lastDay);
        from = DateTime(_selectedYear, _selectedMonth, d);
        to = from;
        break;
      case TimeMachineMode.any:
        from = _fromDate;
        to = _toDate;
        break;
    }
    _fromDate = from;
    _toDate = to;
  }

  void _notifyRange() {
    widget.onRangeChanged(_fromDate, _toDate);
  }

  void _onModeSelected(TimeMachineMode mode) {
    if (mode == _mode) return;
    HapticFeedback.selectionClick();
    setState(() {
      _mode = mode;
      _syncDisplayToSelected();
      _applyCurrentRange();
      _notifyRange();
    });
  }

  /// Date for the ball display: follows scroll (display) until release, then equals committed selection.
  DateTime get _displayDate =>
      DateTime(_displayYear, _displayMonth, _displayDay);

  void _syncDisplayToSelected() {
    _displayYear = _selectedYear;
    _displayMonth = _selectedMonth;
    _displayDay = _selectedDay;
  }

  void _commitRulerChange(void Function() apply) {
    setState(() {
      apply();
      _syncDisplayToSelected();
      _applyCurrentRange();
      _notifyRange();
    });
  }

  @override
  void initState() {
    super.initState();
    _applyCurrentRange();
    WidgetsBinding.instance.addPostFrameCallback((_) => _notifyRange());
  }

  @override
  void didUpdateWidget(TimeRangePicker oldWidget) {
    super.didUpdateWidget(oldWidget);
    final earliest = widget.earliestDate;
    final needFix = earliest != null && _selectedYear < earliest.year;
    final monthClamped = _selectedMonth.clamp(1, 12);
    final lastDay = DateTime(_selectedYear, monthClamped + 1, 0).day;
    final dayClamped = _selectedDay.clamp(1, lastDay);
    final needNormalize = monthClamped != _selectedMonth || dayClamped != _selectedDay;
    if (needFix || needNormalize) {
      setState(() {
        if (needFix) _selectedYear = earliest!.year;
        _selectedMonth = monthClamped;
        _selectedDay = dayClamped;
        _syncDisplayToSelected();
        _applyCurrentRange();
        _notifyRange();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final rulerChild = _mode != TimeMachineMode.any
        ? TimeRuler(
            mode: _mode,
            selectedYear: _selectedYear,
            selectedMonth: _selectedMonth,
            selectedDay: _selectedDay,
            earliest: widget.earliestDate,
            onYearChanged: (y) => _commitRulerChange(() => _selectedYear = y),
            onMonthChanged: (m) => _commitRulerChange(() => _selectedMonth = m),
            onDayChanged: (d) => _commitRulerChange(() => _selectedDay = d),
            onDisplayMonthChanged: (y, m) {
              setState(() {
                _displayYear = y;
                _displayMonth = m;
                _displayDay = _selectedDay;
              });
            },
            onDisplayDayChanged: (y, m, d) {
              setState(() {
                _displayYear = y;
                _displayMonth = m;
                _displayDay = d;
              });
            },
          )
        : TimeRangeOverlayPicker(
            fromDate: _fromDate,
            toDate: _toDate,
            earliest: widget.earliestDate,
            onFromChanged: (d) {
              setState(() {
                _fromDate = d;
                if (_toDate.isBefore(_fromDate)) _toDate = _fromDate;
                _notifyRange();
              });
            },
            onToChanged: (d) {
              setState(() {
                _toDate = d;
                if (_fromDate.isAfter(_toDate)) _fromDate = _toDate;
                _notifyRange();
              });
            },
          );

    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        CustomPopup(
          position: PopupPosition.top,
          verticalOffset: 12,
          contentRadius: 12,
          barrierColor: Colors.transparent,
          content: PointerInterceptor(
            child: _TimeMachineModeMenu(
              currentMode: _mode,
              onSelect: (mode) {
                _onModeSelected(mode);
              },
            ),
          ),
          child: PointerInterceptor(
            child: TimeRangeControllerBall(
              key:
                  ValueKey('ball-$_displayYear-$_displayMonth-$_displayDay'),
              mode: _mode,
              selectedDate: _displayDate,
              loading: widget.loading,
            ),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: SizedBox(
            height: _kPickerBlockHeight,
            child: rulerChild,
          ),
        ),
      ],
    );
  }
}

/// Mode selection popup: single-select menu (like [LayerButton]); closes on selection.
class _TimeMachineModeMenu extends StatelessWidget {
  final TimeMachineMode currentMode;
  final void Function(TimeMachineMode) onSelect;

  const _TimeMachineModeMenu({
    required this.currentMode,
    required this.onSelect,
  });

  static const _itemKeys = [
    (TimeMachineMode.year, 'time_machine.menu_year'),
    (TimeMachineMode.month, 'time_machine.menu_month'),
    (TimeMachineMode.day, 'time_machine.menu_day'),
    (TimeMachineMode.any, 'time_machine.menu_any'),
  ];

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: _itemKeys
          .map(
            (e) => InkWell(
              onTap: () {
                HapticFeedback.selectionClick();
                onSelect(e.$1);
                Navigator.of(context).pop();
              },
              borderRadius: BorderRadius.circular(8),
              child: Padding(
                padding:
                    const EdgeInsets.symmetric(vertical: 10, horizontal: 12),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (e.$1 == currentMode)
                      Icon(
                        Icons.check,
                        size: 18,
                        color: StyleConstants.defaultColor,
                      )
                    else
                      const SizedBox(width: 18, height: 18),
                    const SizedBox(width: 8),
                    Text(
                      context.tr(e.$2),
                      style: TextStyle(
                        color: e.$1 == currentMode
                            ? StyleConstants.defaultColor
                            : Colors.white70,
                        fontSize: 14,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          )
          .toList(),
    );
  }
}

/// Mode button: square, semi-transparent (matches timeline style); tap opens [CustomPopup] menu.
/// Day mode: only year-month (day is on ruler); month mode: only year (month is on ruler).
class TimeRangeControllerBall extends StatelessWidget {
  final TimeMachineMode mode;
  final DateTime selectedDate;
  final bool loading;

  const TimeRangeControllerBall({
    super.key,
    required this.mode,
    required this.selectedDate,
    required this.loading,
  });

  static const double _buttonSize = 60;
  static const double _borderRadius = 12;
  static const double _emphasisFontSize = 13;

  static final TextStyle _contentStyle = TextStyle(
    color: Colors.white,
    fontSize: _emphasisFontSize,
    fontWeight: FontWeight.w600,
  );

  @override
  Widget build(BuildContext context) {
    final y = selectedDate.year;
    final m = selectedDate.month.toString().padLeft(2, '0');
    // Only show what the ruler doesn't: day mode -> year-month; month mode -> year; year mode -> year; any -> mode label only.
    final String text = switch (mode) {
      TimeMachineMode.year => '$y',
      TimeMachineMode.month => '$y',
      TimeMachineMode.day => '$y-$m',
      TimeMachineMode.any => context.tr('time_machine.menu_any'),
    };
    final content = Text(text, style: _contentStyle);

    return ClipRRect(
      borderRadius: BorderRadius.circular(_borderRadius),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
        child: Container(
          width: _buttonSize,
          height: _buttonSize,
          decoration: BoxDecoration(
            color: Colors.white.withValues(alpha: 0.2),
            borderRadius: BorderRadius.circular(_borderRadius),
            border: Border.all(
              color: Colors.white.withValues(alpha: 0.35),
              width: 1,
            ),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.12),
                blurRadius: 12,
                offset: const Offset(0, 2),
              ),
            ],
          ),
          child: Stack(
            alignment: Alignment.center,
            children: [
              FittedBox(
                fit: BoxFit.scaleDown,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                  child: content,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Ruler height (matches RulerScale rulerExtent); compact but readable.
const double _kRulerExtent = 44.0;
const double _kRulerUnitSpacing = 36.0;

/// Fixed height for ruler / any picker block; must fit both ruler and any-mode date tiles.
const double _kPickerBlockHeight = 60.0;

/// Delay before snap-after-release to avoid clashing with inertia scroll.
const Duration _kSnapDelay = Duration(milliseconds: 100);

const EdgeInsets _kRulerMargin = EdgeInsets.symmetric(horizontal: 16);

TextStyle _rulerLabelTextStyle() => TextStyle(
      color: Colors.white.withValues(alpha: 0.9),
      fontSize: 11,
    );

Widget _buildRulerContainer(Widget child) {
  return ClipRRect(
    borderRadius: BorderRadius.circular(12),
    child: BackdropFilter(
      filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
      child: Container(
        decoration: BoxDecoration(
          color: Colors.white.withValues(alpha: 0.15),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(
            color: Colors.white.withValues(alpha: 0.2),
            width: 1,
          ),
        ),
        child: child,
      ),
    ),
  );
}

class TimeRuler extends StatelessWidget {
  final TimeMachineMode mode;
  final int selectedYear;
  final int selectedMonth;
  final int selectedDay;
  final DateTime? earliest;
  final void Function(int) onYearChanged;
  final void Function(int) onMonthChanged;
  final void Function(int) onDayChanged;
  final void Function(int year, int month)? onDisplayMonthChanged;
  final void Function(int year, int month, int day)? onDisplayDayChanged;

  const TimeRuler({
    super.key,
    required this.mode,
    required this.selectedYear,
    required this.selectedMonth,
    required this.selectedDay,
    required this.earliest,
    required this.onYearChanged,
    required this.onMonthChanged,
    required this.onDayChanged,
    this.onDisplayMonthChanged,
    this.onDisplayDayChanged,
  });

  @override
  Widget build(BuildContext context) {
    if (mode == TimeMachineMode.year) {
      final endYear = DateTime.now().year;
      // When earliest is set, show [earliest.year, current year]; otherwise only current year.
      final startYear = earliest != null ? earliest!.year : endYear;
      final yearCount = endYear - startYear + 1;
      final years = List.generate(yearCount, (i) => startYear + i);
      final labels = years.map((y) => '$y').toList();
      return _SnapRulerScaleRuler(
        key: ValueKey('year-$yearCount'),
        labels: labels,
        selectedIndex: (selectedYear - startYear).clamp(0, yearCount - 1),
        onSelect: (i) => onYearChanged(startYear + i),
      );
    }

    if (mode == TimeMachineMode.month) {
      final earliestDate = earliest ?? DateTime(DateTime.now().year - 1, 1, 1);
      return _InfiniteTimeRuler(
        key: const ValueKey('infinite-month'),
        isMonthMode: true,
        earliest: earliestDate,
        selectedYear: selectedYear,
        selectedMonth: selectedMonth,
        selectedDay: selectedDay,
        onMonthSelected: (y, m) {
          onYearChanged(y);
          onMonthChanged(m);
        },
        onDaySelected: (y, m, d) {
          onYearChanged(y);
          onMonthChanged(m);
          onDayChanged(d);
        },
        onDisplayMonthChanged: onDisplayMonthChanged,
        onDisplayDayChanged: onDisplayDayChanged,
      );
    }

    if (mode == TimeMachineMode.day) {
      final earliestDate = earliest ?? DateTime(DateTime.now().year - 1, 1, 1);
      return _InfiniteTimeRuler(
        key: const ValueKey('infinite-day'),
        isMonthMode: false,
        earliest: earliestDate,
        selectedYear: selectedYear,
        selectedMonth: selectedMonth,
        selectedDay: selectedDay,
        onMonthSelected: (y, m) {
          onYearChanged(y);
          onMonthChanged(m);
        },
        onDaySelected: (y, m, d) {
          onYearChanged(y);
          onMonthChanged(m);
          onDayChanged(d);
        },
        onDisplayMonthChanged: onDisplayMonthChanged,
        onDisplayDayChanged: onDisplayDayChanged,
      );
    }

    return const SizedBox.shrink();
  }
}

/// Infinite scroll time ruler: ListView.builder with a sliding window for month/day.
/// Both modes show a window around the selected value; on release the ruler regenerates around the
/// new selection so the user can keep scrolling in either direction (infinite feel without building all items).
/// Month: window = selected month ± _monthWindowHalfMonths (global month indices, clamped to earliest..now).
/// Day: window = selected date ± _dayWindowHalfDays (clamped to earliest..today). Each day is
/// start.add(Duration(days: i)), so month lengths (28/29/30/31) and Feb leap years are correct.
class _InfiniteTimeRuler extends StatefulWidget {
  final bool isMonthMode;
  final DateTime earliest;
  final int selectedYear;
  final int selectedMonth;
  final int selectedDay;
  final void Function(int year, int month) onMonthSelected;
  final void Function(int year, int month, int day) onDaySelected;
  final void Function(int year, int month)? onDisplayMonthChanged;
  final void Function(int year, int month, int day)? onDisplayDayChanged;

  const _InfiniteTimeRuler({
    super.key,
    required this.isMonthMode,
    required this.earliest,
    required this.selectedYear,
    required this.selectedMonth,
    required this.selectedDay,
    required this.onMonthSelected,
    required this.onDaySelected,
    this.onDisplayMonthChanged,
    this.onDisplayDayChanged,
  });

  @override
  State<_InfiniteTimeRuler> createState() => _InfiniteTimeRulerState();
}

class _InfiniteTimeRulerState extends State<_InfiniteTimeRuler> {
  late ScrollController _scrollController;
  Timer? _snapTimer;
  bool _isScrolling = false;
  int _lastReportedIndex = -1;
  int _lastHapticIndex = -1; // Haptic when crossing a tick, same as year mode
  double _viewportWidth = 0;

  /// Sliding window half-size: on release ruler regenerates around new selection to feel infinite.
  static const int _monthWindowHalfMonths = 90;
  static const int _dayWindowHalfDays = 90;

  // ----- Month mode: window = [center - half, center + half] in global month indices -----
  int get _monthCenterIndex {
    final earliestYear = widget.earliest.year;
    return (widget.selectedYear - earliestYear) * 12 + (widget.selectedMonth - 1);
  }

  /// Total months from earliest year Jan up to and including current month (no future months).
  int get _monthTotalCount {
    final nowYear = DateTime.now().year;
    final nowMonth = DateTime.now().month;
    final earliestYear = widget.earliest.year;
    final n = (nowYear - earliestYear) * 12 + nowMonth;
    return n < 0 ? 0 : n;
  }

  int get _monthWindowStartIndex {
    final total = _monthTotalCount;
    if (total == 0) return 0;
    final center = _monthCenterIndex.clamp(0, total - 1);
    return (center - _monthWindowHalfMonths).clamp(0, total - 1);
  }

  int get _monthWindowEndIndex {
    final total = _monthTotalCount;
    if (total == 0) return 0;
    final center = _monthCenterIndex.clamp(0, total - 1);
    return (center + _monthWindowHalfMonths).clamp(0, total - 1);
  }

  // ----- Day mode: window = [center - half, center + half] in days -----
  // Uses DateTime +/- Duration(days): calendar and Feb 28/29 (leap year) are handled correctly.
  // Do not clamp day window start to earliest (first trajectory date), or months with no trajectory would fall outside the window and show blank.
  // If selected date is after today (e.g. switched from month mode with a future month), use today as window end so start <= end and ticks are shown.
  // Do not clamp start to (now.year - 2), or past months like 2012-02 would have start clamped to 2023 and yield start > end.
  static DateTime _today() => DateTime(DateTime.now().year, DateTime.now().month, DateTime.now().day);

  /// Normalize day to valid range for the month so e.g. Feb 31 is not interpreted as Mar 2 and the ruler stays on the selected month.
  static DateTime _selectedDateInDayMode(int y, int m, int d) {
    final lastDay = DateTime(y, m + 1, 0).day;
    return DateTime(y, m, d.clamp(1, lastDay));
  }

  DateTime get _dayWindowStart {
    final sel = _selectedDateInDayMode(widget.selectedYear, widget.selectedMonth, widget.selectedDay);
    final today = _today();
    if (sel.isAfter(today)) {
      return today.subtract(const Duration(days: _dayWindowHalfDays * 2));
    }
    return sel.subtract(const Duration(days: _dayWindowHalfDays));
  }

  DateTime get _dayWindowEnd {
    final sel = _selectedDateInDayMode(widget.selectedYear, widget.selectedMonth, widget.selectedDay);
    final today = _today();
    if (sel.isAfter(today)) {
      return today;
    }
    final end = sel.add(const Duration(days: _dayWindowHalfDays));
    return end.isAfter(today) ? today : end;
  }

  // ----- Unified: item count and selected index within current window -----
  int get _itemCount {
    if (widget.isMonthMode) {
      if (_monthTotalCount == 0) return 0;
      final start = _monthWindowStartIndex;
      final end = _monthWindowEndIndex;
      return end - start + 1;
    } else {
      final start = _dayWindowStart;
      final end = _dayWindowEnd;
      final days = end.difference(start).inDays + 1;
      return days < 0 ? 0 : days;
    }
  }

  int get _selectedIndex {
    final maxIdx = _itemCount > 0 ? _itemCount - 1 : 0;
    if (widget.isMonthMode) {
      final start = _monthWindowStartIndex;
      final center = _monthCenterIndex;
      return (center - start).clamp(0, maxIdx);
    } else {
      final start = _dayWindowStart;
      final sel = _selectedDateInDayMode(widget.selectedYear, widget.selectedMonth, widget.selectedDay);
      final days = sel.difference(start).inDays;
      return days.clamp(0, maxIdx);
    }
  }

  void _reportMonth(int indexInWindow) {
    final earliestYear = widget.earliest.year;
    final globalIndex = _monthWindowStartIndex + indexInWindow;
    final year = earliestYear + globalIndex ~/ 12;
    final month = globalIndex % 12 + 1;
    widget.onMonthSelected(year, month);
  }

  void _reportDay(int indexInWindow) {
    final start = _dayWindowStart;
    final d = start.add(Duration(days: indexInWindow));
    widget.onDaySelected(d.year, d.month, d.day);
  }

  /// True if index idx in current window equals current selection (avoid redundant report/rebuild).
  bool _indexEqualsCurrentSelection(int indexInWindow) {
    if (widget.isMonthMode) {
      final earliestYear = widget.earliest.year;
      final globalIndex = _monthWindowStartIndex + indexInWindow;
      final y = earliestYear + globalIndex ~/ 12;
      final m = globalIndex % 12 + 1;
      return y == widget.selectedYear && m == widget.selectedMonth;
    } else {
      final start = _dayWindowStart;
      final d = start.add(Duration(days: indexInWindow));
      final sel = _selectedDateInDayMode(widget.selectedYear, widget.selectedMonth, widget.selectedDay);
      return d.year == sel.year && d.month == sel.month && d.day == sel.day;
    }
  }

  @override
  void initState() {
    super.initState();
    _scrollController = ScrollController();
    _lastReportedIndex = _selectedIndex;
    _lastHapticIndex = _selectedIndex;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients) return;
      final idx = _selectedIndex.clamp(0, _itemCount - 1);
      _scrollController.jumpTo((idx * _kRulerUnitSpacing).toDouble());
    });
  }

  @override
  void didUpdateWidget(_InfiniteTimeRuler oldWidget) {
    super.didUpdateWidget(oldWidget);
    final count = _itemCount;
    if (count <= 0) return;
    final selectionChanged = widget.isMonthMode
        ? (oldWidget.selectedYear != widget.selectedYear || oldWidget.selectedMonth != widget.selectedMonth)
        : (oldWidget.selectedYear != widget.selectedYear ||
            oldWidget.selectedMonth != widget.selectedMonth ||
            oldWidget.selectedDay != widget.selectedDay);
    if (!_isScrolling && selectionChanged) {
      final idx = _selectedIndex.clamp(0, count - 1);
      _lastReportedIndex = idx;
      _lastHapticIndex = idx;
      if (_scrollController.hasClients) {
        _scrollController.jumpTo((idx * _kRulerUnitSpacing).toDouble());
      }
    }
  }

  @override
  void dispose() {
    _snapTimer?.cancel();
    _scrollController.dispose();
    super.dispose();
  }

  int _indexAtScrollOffset(double scrollOffset) {
    final maxIdx = _itemCount > 0 ? _itemCount - 1 : 0;
    if (_viewportWidth <= 0) return (scrollOffset / _kRulerUnitSpacing).round().clamp(0, maxIdx);
    final centerPadding = _viewportWidth / 2 - _kRulerUnitSpacing / 2;
    final centerContent = scrollOffset + _viewportWidth / 2;
    final index = ((centerContent - centerPadding - _kRulerUnitSpacing / 2) / _kRulerUnitSpacing).round();
    return index.clamp(0, maxIdx);
  }

  /// Returns a Future that completes when the snap animation finishes.
  Future<void> _snapToIndex(int index) async {
    if (!_scrollController.hasClients) return;
    final offset = (index * _kRulerUnitSpacing).toDouble();
    await _scrollController.animateTo(
      offset,
      duration: const Duration(milliseconds: 200),
      curve: Curves.easeOutCubic,
    );
  }

  /// During scroll: haptic when center crosses a tick; update display (button) only, no commit/reload.
  void _onScrollUpdate(ScrollNotification n) {
    if (_viewportWidth <= 0) return;
    final idx = _indexAtScrollOffset(n.metrics.pixels);
    if (idx != _lastHapticIndex) {
      _lastHapticIndex = idx;
      HapticFeedback.selectionClick();
      if (widget.isMonthMode) {
        final earliestYear = widget.earliest.year;
        final globalIndex = _monthWindowStartIndex + idx;
        final y = earliestYear + globalIndex ~/ 12;
        final m = globalIndex % 12 + 1;
        widget.onDisplayMonthChanged?.call(y, m);
      } else {
        final start = _dayWindowStart;
        final d = start.add(Duration(days: idx));
        widget.onDisplayDayChanged?.call(d.year, d.month, d.day);
      }
    }
  }

  void _onScrollEnd(ScrollNotification n) {
    _isScrolling = false;
    _snapTimer?.cancel();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients) return;
      _snapTimer = Timer(_kSnapDelay, () async {
        if (!mounted || !_scrollController.hasClients) return;
        final idx = _indexAtScrollOffset(_scrollController.offset);
        _lastReportedIndex = idx;
        _lastHapticIndex = idx;
        await _snapToIndex(idx);
        // Only after snap animation completes: report if selection changed → parent updates → ruler regenerates.
        if (!mounted) return;
        if (!_indexEqualsCurrentSelection(idx)) {
          if (widget.isMonthMode) {
            _reportMonth(idx);
          } else {
            _reportDay(idx);
          }
        }
        _snapTimer = null;
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final itemCount = _itemCount;
    if (itemCount <= 0) {
      return SizedBox(
        height: _kRulerExtent,
        child: Padding(
          padding: _kRulerMargin,
          child: _buildRulerContainer(const SizedBox()),
        ),
      );
    }
    final selectedIndex = _selectedIndex.clamp(0, itemCount - 1);
    return SizedBox(
      height: _kRulerExtent,
      child: Padding(
        padding: _kRulerMargin,
        child: _buildRulerContainer(
          LayoutBuilder(
            builder: (context, constraints) {
              final w = constraints.maxWidth;
              if (w > 0 && w != _viewportWidth) {
                WidgetsBinding.instance.addPostFrameCallback((_) {
                  if (mounted) setState(() => _viewportWidth = w);
                });
              }
              final centerPadding = (w / 2 - _kRulerUnitSpacing / 2).clamp(0.0, double.infinity);
              return Stack(
                alignment: Alignment.center,
                children: [
                  NotificationListener<ScrollNotification>(
                    onNotification: (n) {
                      if (n is ScrollStartNotification) _isScrolling = true;
                      if (n is ScrollUpdateNotification) _onScrollUpdate(n);
                      if (n is ScrollEndNotification) _onScrollEnd(n);
                      return false;
                    },
                    child: ListView.builder(
                      controller: _scrollController,
                      scrollDirection: Axis.horizontal,
                      itemExtent: _kRulerUnitSpacing,
                      itemCount: itemCount,
                      padding: EdgeInsets.only(left: centerPadding, right: centerPadding),
                      physics: const AlwaysScrollableScrollPhysics(),
                      itemBuilder: (context, i) {
                        if (widget.isMonthMode) {
                          final earliestYear = widget.earliest.year;
                          final globalIndex = _monthWindowStartIndex + i;
                          final y = earliestYear + globalIndex ~/ 12;
                          final m = globalIndex % 12 + 1;
                          final label = context.tr('time_machine.month_format', args: ['$m']);
                          return _buildTick(label, i == selectedIndex);
                        } else {
                          final start = _dayWindowStart;
                          final d = start.add(Duration(days: i));
                          final label = d.day.toString().padLeft(2, '0');
                          return _buildTick(label, i == selectedIndex);
                        }
                      },
                    ),
                  ),
                  IgnorePointer(
                    child: Center(
                      child: Container(
                        width: 2,
                        height: _kRulerExtent,
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

  Widget _buildTick(String label, bool isSelected) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Container(
          width: 2,
          height: isSelected ? 10 : 6,
          color: isSelected ? StyleConstants.defaultColor : Colors.white.withValues(alpha: 0.5),
        ),
        SizedBox(height: isSelected ? 4 : 6),
        Text(label, style: _rulerLabelTextStyle()),
      ],
    );
  }
}

/// Wrapper around internal [RulerScale] with snap-on-release (100ms delay).
/// Used for year mode only; month/day use [ _InfiniteTimeRuler] for infinite scroll.
class _SnapRulerScaleRuler extends StatefulWidget {
  final List<String> labels;
  final int selectedIndex;
  final void Function(int) onSelect;

  const _SnapRulerScaleRuler({
    super.key,
    required this.labels,
    required this.selectedIndex,
    required this.onSelect,
  });

  @override
  State<_SnapRulerScaleRuler> createState() => _SnapRulerScaleRulerState();
}

class _SnapRulerScaleRulerState extends State<_SnapRulerScaleRuler> {
  final RulerScaleController _controller = RulerScaleController();
  late double _lastReportedValue;
  int _lastReportedIndex =
      -1; // Only call onSelect when tick index changes to avoid duplicate loads on scroll/snap.
  Timer? _snapTimer;
  bool _isScrolling = false;

  @override
  void initState() {
    super.initState();
    final idx = widget.selectedIndex.clamp(0, widget.labels.length - 1);
    _lastReportedValue = idx.toDouble();
    _lastReportedIndex = idx;
  }

  @override
  void didUpdateWidget(_SnapRulerScaleRuler oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Sync ruler position when parent selection changes (e.g. mode switch); do not interrupt while scrolling.
    if (!_isScrolling &&
        oldWidget.selectedIndex != widget.selectedIndex &&
        widget.selectedIndex >= 0 &&
        widget.selectedIndex < widget.labels.length) {
      _lastReportedValue = widget.selectedIndex.toDouble();
      _lastReportedIndex = widget.selectedIndex;
      _controller.jumpToValue(_lastReportedValue);
    }
  }

  void _onScrollEnd() {
    _isScrolling = false;
    _snapTimer?.cancel();
    // Start snap after one frame so the package's postFrameCallback has run and _lastReportedValue is the release position.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _snapTimer = Timer(_kSnapDelay, () {
        if (!mounted) return;
        _controller.jumpToValue(_lastReportedValue);
        final idx = _lastReportedValue.round();
        if (idx != _lastReportedIndex) {
          _lastReportedIndex = idx;
          widget.onSelect(idx);
        }
        _snapTimer = null;
      });
    });
  }

  @override
  void dispose() {
    _snapTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final labels = widget.labels;
    if (labels.length < 2) {
      return SizedBox(
        height: _kRulerExtent,
        child: Padding(
          padding: _kRulerMargin,
          child: _buildRulerContainer(
            Center(
              child: labels.isEmpty
                  ? null
                  : Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Container(
                          width: 2,
                          height: 10,
                          color: StyleConstants.defaultColor,
                        ),
                        const SizedBox(height: 4),
                        Text(labels.first, style: _rulerLabelTextStyle()),
                      ],
                    ),
            ),
          ),
        ),
      );
    }

    final maxValue = (labels.length - 1).toDouble();
    final selectedIndex = widget.selectedIndex.clamp(0, labels.length - 1);

    return SizedBox(
      height: _kRulerExtent,
      child: Padding(
        padding: _kRulerMargin,
        child: _buildRulerContainer(
          RulerScale(
            controller: _controller,
            minValue: 0,
            maxValue: maxValue,
            step: 1,
            majorTickInterval: 1,
            unitSpacing: _kRulerUnitSpacing,
            rulerExtent: _kRulerExtent,
            direction: Axis.horizontal,
            initialValue: selectedIndex.toDouble(),
            useScrollAnimation: true, // Animate on snap-after-release.
            animateInitialScroll:
                false, // Show target value immediately on mode switch, no scroll animation.
            hapticFeedbackEnabled: true,
            showDefaultIndicator: true,
            decoration: null,
            majorTickColor: Colors.white.withValues(alpha: 0.5),
            minorTickColor: Colors.white.withValues(alpha: 0.35),
            selectedTickColor: StyleConstants.defaultColor,
            selectedTickWidth: 2,
            selectedTickLength: 10,
            indicatorColor: StyleConstants.defaultColor,
            indicatorWidth: 2,
            labelStyle: _rulerLabelTextStyle(),
            labelFormatter: (value) => labels[value.round()],
            onValueChanged: (value) {
              _lastReportedValue = value;
              final idx = value.round();
              if (idx != _lastReportedIndex) {
                _lastReportedIndex = idx;
                widget.onSelect(idx);
              }
            },
            onScrollStart: () {
              _isScrolling = true;
              _snapTimer?.cancel();
            },
            onScrollEnd: _onScrollEnd,
          ),
        ),
      ),
    );
  }
}

class TimeRangeOverlayPicker extends StatelessWidget {
  final DateTime fromDate;
  final DateTime toDate;
  final DateTime? earliest;
  final void Function(DateTime) onFromChanged;
  final void Function(DateTime) onToChanged;

  const TimeRangeOverlayPicker({
    super.key,
    required this.fromDate,
    required this.toDate,
    required this.earliest,
    required this.onFromChanged,
    required this.onToChanged,
  });

  static final DateFormat _fmt = DateFormat('yyyy-MM-dd');

  @override
  Widget build(BuildContext context) {
    // Same horizontal alignment as ruler: full width with two equal columns inside.
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(12),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
          child: Container(
            width: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                color: Colors.white.withValues(alpha: 0.2),
                width: 1,
              ),
            ),
            child: Row(
              children: [
                Expanded(
                  child: _TapTile(
                    label: context.tr('journey.start_time'),
                    value: _fmt.format(fromDate),
                    onTap: () => _showDatePicker(context, fromDate, earliest, onFromChanged),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _TapTile(
                    label: context.tr('journey.end_time'),
                    value: _fmt.format(toDate),
                    onTap: () => _showDatePicker(context, toDate, earliest, onToChanged),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  static Future<void> _showDatePicker(
    BuildContext context,
    DateTime initial,
    DateTime? earliestLimit,
    void Function(DateTime) onChanged,
  ) async {
    final last = DateTime.now();
    final rawFirst = earliestLimit ?? DateTime(initial.year - 10);
    final first = rawFirst.isAfter(last) ? last : rawFirst;
    var safeInitial = initial;
    if (safeInitial.isBefore(first)) safeInitial = first;
    if (safeInitial.isAfter(last)) safeInitial = last;
    final picked = await showDatePicker(
      context: context,
      initialDate: safeInitial,
      firstDate: first,
      lastDate: last,
    );
    if (picked != null) onChanged(picked);
  }
}

class _TapTile extends StatelessWidget {
  final String label;
  final String value;
  final VoidCallback onTap;

  const _TapTile({
    required this.label,
    required this.value,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                label,
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.8),
                  fontSize: 10,
                ),
              ),
              const SizedBox(height: 2),
              Text(
                value,
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
