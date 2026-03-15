import 'dart:async';
import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'time_ruler.dart';

export 'time_ruler.dart' show TimeMachineMode, TimeRuler;

/// Clamps (y, m, d) to the valid range for [mode] and [earliest], matching the ruler.
/// When [earliest] is null, only normalizes month/day to valid values.
(int, int, int) clampTimeRulerSelection(
  TimeMachineMode mode,
  DateTime? earliest,
  int y,
  int m,
  int d,
) {
  if (earliest == null) {
    m = m.clamp(1, 12);
    d = d.clamp(1, DateTime(y, m + 1, 0).day);
    return (y, m, d);
  }
  final now = DateTime.now();
  switch (mode) {
    case TimeMachineMode.year:
      y = y.clamp(earliest.year, now.year);
      m = m.clamp(1, 12);
      d = d.clamp(1, DateTime(y, m + 1, 0).day);
      return (y, m, d);
    case TimeMachineMode.month:
      if (y < earliest.year || (y == earliest.year && m < earliest.month)) {
        y = earliest.year;
        m = earliest.month;
      } else if (y > now.year || (y == now.year && m > now.month)) {
        y = now.year;
        m = now.month;
      }
      m = m.clamp(1, 12);
      d = d.clamp(1, DateTime(y, m + 1, 0).day);
      return (y, m, d);
    case TimeMachineMode.day:
      final earliestDay = DateTime(earliest.year, earliest.month, earliest.day);
      final today = DateTime(now.year, now.month, now.day);
      m = m.clamp(1, 12);
      d = d.clamp(1, DateTime(y, m + 1, 0).day);
      final sel = DateTime(y, m, d);
      if (sel.isBefore(earliestDay)) {
        return (earliest.year, earliest.month, earliest.day);
      }
      if (sel.isAfter(today)) {
        return (now.year, now.month, now.day);
      }
      return (y, m, d);
    case TimeMachineMode.any:
      m = m.clamp(1, 12);
      d = d.clamp(1, DateTime(y, m + 1, 0).day);
      return (y, m, d);
  }
}

api.LayerFilter ensureTimeMachineDefaultKind(api.LayerFilter f) {
  if (f.defaultKind) return f;
  return api.LayerFilter(
    currentJourney: f.currentJourney,
    defaultKind: true,
    flightKind: f.flightKind,
  );
}

/// Time range picker: ball + year/month/day ruler or any date-range overlay.
/// Reports the selected [from]-[to] range to the parent via [onRangeChanged].
class TimeRangePicker extends StatefulWidget {
  final DateTime? earliestDate;
  final bool loading;
  final void Function(DateTime from, DateTime to) onRangeChanged;
  final api.LayerFilter? timeMachineLayerFilter;
  final void Function(api.LayerFilter)? onLayerFilterChanged;

  const TimeRangePicker({
    super.key,
    this.earliestDate,
    this.loading = false,
    required this.onRangeChanged,
    this.timeMachineLayerFilter,
    this.onLayerFilterChanged,
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
      _applySelectionInRange();
    });
  }

  /// If current selection is outside [earliest] range, clamps it and syncs display/range. Returns true if changed.
  bool _applySelectionInRange() {
    final earliest = widget.earliestDate;
    if (earliest == null) return false;
    final (cy, cm, cd) = clampTimeRulerSelection(
      _mode,
      earliest,
      _selectedYear,
      _selectedMonth,
      _selectedDay,
    );
    if (cy == _selectedYear && cm == _selectedMonth && cd == _selectedDay) {
      return false;
    }
    _selectedYear = cy;
    _selectedMonth = cm;
    _selectedDay = cd;
    _updateDisplay(cy, cm, cd);
    _applyCurrentRange();
    _notifyRange();
    return true;
  }

  DateTime get _displayDate =>
      DateTime(_displayYear, _displayMonth, _displayDay);

  void _updateDisplay(int y, [int? m, int? d]) {
    _displayYear = y;
    _displayMonth = m ?? _selectedMonth;
    _displayDay = d ?? _selectedDay;
  }

  void _commitRulerChange(void Function() apply) {
    setState(() {
      apply();
      _updateDisplay(_selectedYear, _selectedMonth, _selectedDay);
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
    if (_applySelectionInRange()) setState(() {});
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
            onSelectionChanged: (selection) => _commitRulerChange(() {
              _selectedYear = selection.$1;
              if (selection.$2 != null) _selectedMonth = selection.$2!;
              if (selection.$3 != null) _selectedDay = selection.$3!;
            }),
            onDisplayChanged: (s) =>
                setState(() => _updateDisplay(s.$1, s.$2, s.$3)),
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
            child: _TimeMachineModeAndLayerMenu(
              currentMode: _mode,
              onModeSelect: _onModeSelected,
              layerFilter: ensureTimeMachineDefaultKind(
                  widget.timeMachineLayerFilter ??
                      api.getCurrentMainMapLayerFilter()),
              onLayerFilterChanged: widget.onLayerFilterChanged,
            ),
          ),
          child: PointerInterceptor(
            child: TimeRangeControllerBall(
              key: ValueKey('ball-$_displayYear-$_displayMonth-$_displayDay'),
              mode: _mode,
              selectedDate: _displayDate,
              loading: widget.loading,
            ),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: PointerInterceptor(
            child: SizedBox(
              height: _kPickerBlockHeight,
              child: rulerChild,
            ),
          ),
        ),
      ],
    );
  }
}

/// Mode + layer popup: left column = 4 modes (single-select, closes on tap), right column = 2 layers (multi-select), vertical divider between.
class _TimeMachineModeAndLayerMenu extends StatefulWidget {
  final TimeMachineMode currentMode;
  final void Function(TimeMachineMode) onModeSelect;
  final api.LayerFilter layerFilter;
  final void Function(api.LayerFilter)? onLayerFilterChanged;

  const _TimeMachineModeAndLayerMenu({
    required this.currentMode,
    required this.onModeSelect,
    required this.layerFilter,
    this.onLayerFilterChanged,
  });

  @override
  State<_TimeMachineModeAndLayerMenu> createState() =>
      _TimeMachineModeAndLayerMenuState();
}

class _TimeMachineModeAndLayerMenuState
    extends State<_TimeMachineModeAndLayerMenu> {
  static const _modeKeys = [
    (TimeMachineMode.year, 'time_machine.menu_year'),
    (TimeMachineMode.month, 'time_machine.menu_month'),
    (TimeMachineMode.day, 'time_machine.menu_day'),
    (TimeMachineMode.any, 'time_machine.menu_any'),
  ];

  static const _layerKeys = [
    (_LayerKind.default_, 'journey_kind.default'),
    (_LayerKind.flight, 'journey_kind.flight'),
  ];

  late api.LayerFilter _localFilter;
  Timer? _layerTimer;

  @override
  void initState() {
    super.initState();
    _localFilter = api.LayerFilter(
      currentJourney: widget.layerFilter.currentJourney,
      defaultKind: widget.layerFilter.defaultKind,
      flightKind: widget.layerFilter.flightKind,
    );
  }

  @override
  void didUpdateWidget(covariant _TimeMachineModeAndLayerMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.layerFilter != widget.layerFilter) {
      _localFilter = api.LayerFilter(
        currentJourney: widget.layerFilter.currentJourney,
        defaultKind: widget.layerFilter.defaultKind,
        flightKind: widget.layerFilter.flightKind,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return IntrinsicHeight(
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              _buildColumnTitle(context.tr('time_machine.menu_title_time')),
              ..._modeKeys.map((e) => _buildModeItem(e.$1, e.$2)),
            ],
          ),
          VerticalDivider(
            width: 1,
            thickness: 1,
            color: Colors.white24,
            indent: 8,
            endIndent: 8,
          ),
          Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              _buildColumnTitle(context.tr('time_machine.menu_title_layer')),
              ..._layerKeys.map((e) => _buildLayerItem(e.$1, e.$2)),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildColumnTitle(String text) {
    return Padding(
      padding: const EdgeInsets.only(left: 12, right: 12, top: 8, bottom: 4),
      child: Text(
        text,
        textAlign: TextAlign.center,
        style: TextStyle(
          color: Colors.white54,
          fontSize: 12,
        ),
      ),
    );
  }

  Widget _buildMenuTile(BuildContext context, String labelKey, bool isSelected,
      VoidCallback onTap) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 12),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (isSelected)
              Icon(Icons.check, size: 18, color: StyleConstants.defaultColor)
            else
              const SizedBox(width: 18, height: 18),
            const SizedBox(width: 8),
            Text(
              context.tr(labelKey),
              style: TextStyle(
                color:
                    isSelected ? StyleConstants.defaultColor : Colors.white70,
                fontSize: 14,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildModeItem(TimeMachineMode mode, String labelKey) {
    return _buildMenuTile(
      context,
      labelKey,
      mode == widget.currentMode,
      () {
        HapticFeedback.selectionClick();
        widget.onModeSelect(mode);
        Navigator.of(context).pop();
      },
    );
  }

  Widget _buildLayerItem(_LayerKind kind, String labelKey) {
    final isSelected = kind == _LayerKind.default_
        ? _localFilter.defaultKind
        : _localFilter.flightKind;
    return _buildMenuTile(
      context,
      labelKey,
      isSelected,
      () {
        HapticFeedback.selectionClick();
        setState(() {
          _localFilter = api.LayerFilter(
            currentJourney: _localFilter.currentJourney,
            defaultKind: kind == _LayerKind.default_
                ? !_localFilter.defaultKind
                : _localFilter.defaultKind,
            flightKind: kind == _LayerKind.flight
                ? !_localFilter.flightKind
                : _localFilter.flightKind,
          );
        });
        _layerTimer?.cancel();
        _layerTimer = Timer(const Duration(milliseconds: 600), () {
          _layerTimer = null;
          widget.onLayerFilterChanged?.call(_localFilter);
        });
      },
    );
  }

  @override
  void dispose() {
    _layerTimer?.cancel();
    super.dispose();
  }
}

enum _LayerKind { default_, flight }

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

/// Fixed height for ruler / any picker block; must fit both ruler and any-mode date tiles.
const double _kPickerBlockHeight = 60.0;

Widget _buildGlassPanel(Widget child, {EdgeInsets? padding}) {
  return ClipRRect(
    borderRadius: BorderRadius.circular(12),
    child: BackdropFilter(
      filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
      child: Container(
        padding: padding,
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
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: _buildGlassPanel(
        Row(
          children: [
            Expanded(
              child: _TapTile(
                label: context.tr('journey.start_time'),
                value: _fmt.format(fromDate),
                onTap: () =>
                    _showDatePicker(context, fromDate, earliest, onFromChanged),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _TapTile(
                label: context.tr('journey.end_time'),
                value: _fmt.format(toDate),
                onTap: () =>
                    _showDatePicker(context, toDate, earliest, onToChanged),
              ),
            ),
          ],
        ),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
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
