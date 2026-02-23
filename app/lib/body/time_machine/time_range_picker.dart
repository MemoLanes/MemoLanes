import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:memolanes/body/time_machine/advance_ruler_slider.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
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
        from = DateTime(_selectedYear, _selectedMonth, _selectedDay);
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
      _applyCurrentRange();
      _notifyRange();
    });
  }

  /// Currently selected date for the ball display (no hardcoded year/month/day for i18n).
  DateTime get _selectedDate =>
      DateTime(_selectedYear, _selectedMonth, _selectedDay);

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
    if (earliest != null && _selectedYear < earliest.year) {
      setState(() {
        _selectedYear = earliest.year;
        _applyCurrentRange();
        _notifyRange();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.end,
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
                  ValueKey('ball-$_selectedYear-$_selectedMonth-$_selectedDay'),
              mode: _mode,
              selectedDate: _selectedDate,
              loading: widget.loading,
            ),
          ),
        ),
        const SizedBox(height: 12),
        SizedBox(
          height: _kPickerBlockHeight,
          child: Center(
            child: _mode != TimeMachineMode.any
                ? TimeRuler(
                    mode: _mode,
                    selectedYear: _selectedYear,
                    selectedMonth: _selectedMonth,
                    selectedDay: _selectedDay,
                    earliest: widget.earliestDate,
                    onYearChanged: (y) {
                      setState(() {
                        _selectedYear = y;
                        _applyCurrentRange();
                        _notifyRange();
                      });
                    },
                    onMonthChanged: (m) {
                      setState(() {
                        _selectedMonth = m;
                        _applyCurrentRange();
                        _notifyRange();
                      });
                    },
                    onDayChanged: (d) {
                      setState(() {
                        _selectedDay = d;
                        _applyCurrentRange();
                        _notifyRange();
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
                  ),
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

/// Mode button: shows current selection with typography hierarchy; tap opens [CustomPopup] menu.
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

  static const double _ballSize = 88;
  static const double _subFontSize = 9;
  static const double _contextFontSize = 11;
  static const double _emphasisFontSize = 17;

  @override
  Widget build(BuildContext context) {
    final y = selectedDate.year;
    final m = selectedDate.month.toString().padLeft(2, '0');
    final d = selectedDate.day.toString().padLeft(2, '0');
    final contentColor = Colors.grey.shade900;
    final contextColor = Colors.grey.shade700;

    final modeHint = switch (mode) {
      TimeMachineMode.year => context.tr('time_machine.menu_year'),
      TimeMachineMode.month => context.tr('time_machine.menu_month'),
      TimeMachineMode.day => context.tr('time_machine.menu_day'),
      TimeMachineMode.any => context.tr('time_machine.menu_any'),
    };

    Widget content;
    switch (mode) {
      case TimeMachineMode.year:
        content = Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              modeHint,
              style: TextStyle(
                color: contextColor,
                fontSize: _subFontSize,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(height: 2),
            Text(
              '$y',
              style: TextStyle(
                color: contentColor,
                fontSize: _emphasisFontSize,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        );
        break;
      case TimeMachineMode.month:
        content = Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              modeHint,
              style: TextStyle(
                color: contextColor,
                fontSize: _subFontSize,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(height: 2),
            Text(
              '$y',
              style: TextStyle(
                color: contextColor,
                fontSize: _contextFontSize,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(height: 1),
            Text(
              m,
              style: TextStyle(
                color: contentColor,
                fontSize: _emphasisFontSize,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        );
        break;
      case TimeMachineMode.day:
        content = Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              modeHint,
              style: TextStyle(
                color: contextColor,
                fontSize: _subFontSize,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(height: 2),
            Text(
              '$y-$m',
              style: TextStyle(
                color: contextColor,
                fontSize: _contextFontSize,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(height: 1),
            Text(
              d,
              style: TextStyle(
                color: contentColor,
                fontSize: _emphasisFontSize,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        );
        break;
      case TimeMachineMode.any:
        content = Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              modeHint,
              style: TextStyle(
                color: contextColor,
                fontSize: _subFontSize,
                fontWeight: FontWeight.w500,
              ),
            ),
            const SizedBox(height: 2),
            Text(
              context.tr('time_machine.menu_any'),
              style: TextStyle(
                color: contentColor,
                fontSize: _emphasisFontSize,
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        );
        break;
    }

    return Container(
      width: _ballSize,
      height: _ballSize,
      decoration: BoxDecoration(
        color: StyleConstants.defaultColor,
        shape: BoxShape.circle,
        border: Border.all(
          color: StyleConstants.defaultColor.withValues(alpha: 0.7),
          width: 2,
        ),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.08),
            blurRadius: 20,
            offset: const Offset(0, 4),
          ),
        ],
      ),
      child: Stack(
        alignment: Alignment.center,
        children: [
          FittedBox(
            fit: BoxFit.scaleDown,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: content,
            ),
          ),
        ],
      ),
    );
  }
}

/// Ruler height (matches RulerScale rulerExtent).
const double _kRulerExtent = 52.0;
const double _kRulerUnitSpacing = 36.0;

/// Fixed height for ruler / any picker block so the mode button position stays consistent.
const double _kPickerBlockHeight = 88.0;

/// Delay before snap-after-release to avoid clashing with inertia scroll.
const Duration _kSnapDelay = Duration(milliseconds: 100);

class TimeRuler extends StatelessWidget {
  final TimeMachineMode mode;
  final int selectedYear;
  final int selectedMonth;
  final int selectedDay;
  final DateTime? earliest;
  final void Function(int) onYearChanged;
  final void Function(int) onMonthChanged;
  final void Function(int) onDayChanged;

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
      final labels = List.generate(
        12,
        (i) => context.tr('time_machine.month_format', args: ['${i + 1}']),
      );
      return _SnapRulerScaleRuler(
        key: const ValueKey('month-12'),
        labels: labels,
        selectedIndex: (selectedMonth - 1).clamp(0, 11),
        onSelect: (i) => onMonthChanged(i + 1),
      );
    }

    if (mode == TimeMachineMode.day) {
      final daysInMonth = DateTime(selectedYear, selectedMonth + 1, 0).day;
      final labels =
          List.generate(daysInMonth, (i) => (i + 1).toString().padLeft(2, '0'));
      return _SnapRulerScaleRuler(
        key: ValueKey('day-$daysInMonth'),
        labels: labels,
        selectedIndex: (selectedDay - 1).clamp(0, daysInMonth - 1),
        onSelect: (i) => onDayChanged(i + 1),
      );
    }

    return const SizedBox.shrink();
  }
}

/// Wrapper around internal [RulerScale] with snap-on-release (100ms delay).
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

  /// Same style as any-mode date picker: frosted glass, radius 12, white border.
  static Widget _rulerContainer({required Widget child}) {
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

  static const EdgeInsets _rulerMargin = EdgeInsets.symmetric(horizontal: 16);

  static TextStyle get _rulerLabelStyle => TextStyle(
        color: Colors.white.withValues(alpha: 0.9),
        fontSize: 11,
      );

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
          padding: _rulerMargin,
          child: _rulerContainer(
            child: Center(
              child: labels.isEmpty
                  ? null
                  : Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Container(
                          width: 2,
                          height: 12,
                          color: StyleConstants.defaultColor,
                        ),
                        const SizedBox(height: 6),
                        Text(labels.first, style: _rulerLabelStyle),
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
        padding: _rulerMargin,
        child: _rulerContainer(
          child: RulerScale(
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
            selectedTickLength: 12,
            indicatorColor: StyleConstants.defaultColor,
            indicatorWidth: 2,
            labelStyle: _rulerLabelStyle,
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
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
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
                    onTap: () =>
                        _showDatePicker(context, fromDate, onFromChanged),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _TapTile(
                    label: context.tr('journey.end_time'),
                    value: _fmt.format(toDate),
                    onTap: () => _showDatePicker(context, toDate, onToChanged),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _showDatePicker(
    BuildContext context,
    DateTime initial,
    void Function(DateTime) onChanged,
  ) async {
    final last = DateTime.now();
    final rawFirst = earliest ?? DateTime(initial.year - 10);
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
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                label,
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.8),
                  fontSize: 11,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                value,
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 14,
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
