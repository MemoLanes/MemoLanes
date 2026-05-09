import 'dart:async';
import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'time_ruler.dart';
import 'package:memolanes/src/rust/journey_header.dart';

export 'time_ruler.dart' show TimeRulerMode, TimeRuler;

/// Time range picker with mode-based range selection.
/// Supports the [TimeMachineViewMode.period],
/// [TimeMachineViewMode.asOf], and [TimeMachineViewMode.custom] views,
/// with [TimeRulerMode] controlling the time granularity shown by the ruler.
/// Reports the selected [from]-[to] range to the parent via [onRangeChanged].
class TimeRangePicker extends StatefulWidget {
  final DateTime? earliestDate;
  final bool loading;
  final void Function(DateTime from, DateTime to) onRangeChanged;
  final Set<JourneyKind> selectedJourneyKinds;
  final void Function(Set<JourneyKind>)? onJourneyKindsChanged;

  const TimeRangePicker({
    super.key,
    this.earliestDate,
    this.loading = false,
    required this.onRangeChanged,
    required this.selectedJourneyKinds,
    this.onJourneyKindsChanged,
  });

  @override
  State<TimeRangePicker> createState() => _TimeRangePickerState();
}

enum TimeMachineViewMode {
  /// Show only the selected period (year/month/day).
  period,

  /// Show cumulative range from earliest to the selected period end.
  asOf,

  /// User picks an arbitrary [from]-[to] range.
  custom,
}

class _TimeRangePickerState extends State<TimeRangePicker> {
  TimeMachineViewMode _viewMode = TimeMachineViewMode.period;
  TimeRulerMode _rulerMode = TimeRulerMode.year;
  int _selectedYear = DateTime.now().year;
  int _selectedMonth = DateTime.now().month;
  int _selectedDay = DateTime.now().day;

  /// Only for button display; updates in real time while scrolling; syncs with selected on release.
  int _displayYear = DateTime.now().year;
  int _displayMonth = DateTime.now().month;
  int _displayDay = DateTime.now().day;
  DateTime _fromDate = DateTime.now();
  DateTime _toDate = DateTime.now();

  /// Single source of truth for lower bound used by ruler/range/pickers.
  DateTime get _effectiveEarliest {
    final now = DateUtils.dateOnly(DateTime.now());
    final fallback = DateTime(now.year - 1, 1, 1);
    final candidate = widget.earliestDate ?? fallback;
    // Guard: if upstream provides a future earliest, cap to "now".
    return candidate.isAfter(now) ? now : candidate;
  }

  (DateTime from, DateTime to) _periodRangeForSelection() {
    switch (_rulerMode) {
      case TimeRulerMode.year:
        return (DateTime(_selectedYear, 1, 1), DateTime(_selectedYear, 12, 31));
      case TimeRulerMode.month:
        return (
          DateTime(_selectedYear, _selectedMonth, 1),
          DateTime(_selectedYear, _selectedMonth + 1, 0),
        );
      case TimeRulerMode.day:
        final lastDay = DateTime(_selectedYear, _selectedMonth + 1, 0).day;
        final d = _selectedDay.clamp(1, lastDay);
        final date = DateTime(_selectedYear, _selectedMonth, d);
        return (date, date);
      case TimeRulerMode.any:
        return (_fromDate, _toDate);
    }
  }

  void _applyCurrentRange() {
    switch (_viewMode) {
      case TimeMachineViewMode.custom:
        return;
      case TimeMachineViewMode.period:
        final period = _periodRangeForSelection();
        _fromDate = period.$1;
        _toDate = period.$2;
        return;
      case TimeMachineViewMode.asOf:
        final period = _periodRangeForSelection();
        _fromDate = _effectiveEarliest;
        _toDate = period.$2;
        if (_toDate.isBefore(_fromDate)) _toDate = _fromDate;
        return;
    }
  }

  void _notifyRange() {
    widget.onRangeChanged(_fromDate, _toDate);
  }

  void _onRulerModeSelected(TimeRulerMode rulerMode) {
    if (rulerMode == _rulerMode) return;
    AppHaptics.selection();
    setState(() {
      _rulerMode = rulerMode;
      _applyCurrentRange();
    });
    _notifyRange();
  }

  void _onViewModeSelected(TimeMachineViewMode viewMode) {
    if (viewMode == _viewMode) return;
    AppHaptics.selection();
    setState(() {
      _viewMode = viewMode;
      _applyCurrentRange();
    });
    _notifyRange();
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
  Widget build(BuildContext context) {
    final rulerChild = _viewMode != TimeMachineViewMode.custom
        ? TimeRuler(
            rulerMode: _rulerMode,
            selectedYear: _selectedYear,
            selectedMonth: _selectedMonth,
            selectedDay: _selectedDay,
            earliest: _effectiveEarliest,
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
            earliest: _effectiveEarliest,
            onFromChanged: (d) {
              setState(() {
                _fromDate = d;
                if (_fromDate.isBefore(_effectiveEarliest)) {
                  _fromDate = _effectiveEarliest;
                }
                if (_toDate.isBefore(_fromDate)) _toDate = _fromDate;
                _notifyRange();
              });
            },
            onToChanged: (d) {
              setState(() {
                _toDate = d;
                if (_toDate.isBefore(_effectiveEarliest)) {
                  _toDate = _effectiveEarliest;
                }
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
            child: _TimeMachineViewModeAndLayerMenu(
              currentViewMode: _viewMode,
              onViewModeSelect: _onViewModeSelected,
              currentRulerMode: _rulerMode,
              onRulerModeSelect: _onRulerModeSelected,
              selectedJourneyKinds: widget.selectedJourneyKinds,
              onJourneyKindsChanged: widget.onJourneyKindsChanged,
            ),
          ),
          child: PointerInterceptor(
            child: TimeRangeControllerBall(
              key: ValueKey('ball-$_displayYear-$_displayMonth-$_displayDay'),
              viewMode: _viewMode,
              rulerMode: _rulerMode,
              selectedDate: _viewMode == TimeMachineViewMode.custom
                  ? _toDate
                  : _displayDate,
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

/// View mode + granularity + layer popup:
/// - left column = view modes (single-select)
/// - middle column = granularity (year/month/day, disabled in custom)
/// - right column = layers (multi-select)
class _TimeMachineViewModeAndLayerMenu extends StatefulWidget {
  final TimeMachineViewMode currentViewMode;
  final void Function(TimeMachineViewMode) onViewModeSelect;
  final TimeRulerMode currentRulerMode;
  final void Function(TimeRulerMode) onRulerModeSelect;
  final Set<JourneyKind> selectedJourneyKinds;
  final void Function(Set<JourneyKind>)? onJourneyKindsChanged;

  const _TimeMachineViewModeAndLayerMenu({
    required this.currentViewMode,
    required this.onViewModeSelect,
    required this.currentRulerMode,
    required this.onRulerModeSelect,
    required this.selectedJourneyKinds,
    this.onJourneyKindsChanged,
  });

  @override
  State<_TimeMachineViewModeAndLayerMenu> createState() =>
      _TimeMachineViewModeAndLayerMenuState();
}

class _TimeMachineViewModeAndLayerMenuState
    extends State<_TimeMachineViewModeAndLayerMenu> {
  static const _viewModeKeys = [
    (TimeMachineViewMode.period, 'time_machine.menu_view_period'),
    (TimeMachineViewMode.asOf, 'time_machine.menu_view_as_of'),
    (TimeMachineViewMode.custom, 'time_machine.menu_view_custom'),
  ];

  static const _granularityKeys = [
    (TimeRulerMode.year, 'time_machine.menu_year'),
    (TimeRulerMode.month, 'time_machine.menu_month'),
    (TimeRulerMode.day, 'time_machine.menu_day'),
  ];

  static const _layerKeys = [
    (JourneyKind.defaultKind, 'journey_kind.default'),
    (JourneyKind.flight, 'journey_kind.flight'),
  ];

  late Set<JourneyKind> _localKinds;
  late TimeMachineViewMode _localViewMode;
  late TimeRulerMode _localRulerMode;
  Timer? _layerTimer;

  @override
  void initState() {
    super.initState();
    _localKinds = Set.from(widget.selectedJourneyKinds);
    _localViewMode = widget.currentViewMode;
    _localRulerMode = widget.currentRulerMode;
  }

  @override
  void didUpdateWidget(covariant _TimeMachineViewModeAndLayerMenu oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.selectedJourneyKinds != widget.selectedJourneyKinds) {
      _localKinds = Set.from(widget.selectedJourneyKinds);
    }
    if (oldWidget.currentViewMode != widget.currentViewMode) {
      _localViewMode = widget.currentViewMode;
    }
    if (oldWidget.currentRulerMode != widget.currentRulerMode) {
      _localRulerMode = widget.currentRulerMode;
    }
  }

  @override
  Widget build(BuildContext context) {
    final content = IntrinsicHeight(
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              _buildColumnTitle(context.tr('time_machine.menu_title_view')),
              ..._viewModeKeys.map((e) => _buildViewModeItem(e.$1, e.$2)),
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
              _buildColumnTitle(
                  context.tr('time_machine.menu_title_granularity')),
              ..._granularityKeys.map((e) => _buildGranularityItem(e.$1, e.$2)),
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

    // Popup may have a narrow max width (small screens). Allow horizontal scroll
    // instead of letting the Row overflow.
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: content,
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

  Widget _buildViewModeItem(TimeMachineViewMode mode, String labelKey) {
    return _buildMenuTile(
      context,
      labelKey,
      mode == _localViewMode,
      () {
        AppHaptics.selection();
        setState(() => _localViewMode = mode);
        widget.onViewModeSelect(mode);
      },
    );
  }

  Widget _buildGranularityItem(TimeRulerMode rulerMode, String labelKey) {
    final disabled = _localViewMode == TimeMachineViewMode.custom;
    return Opacity(
      opacity: disabled ? 0.28 : 1,
      child: IgnorePointer(
        ignoring: disabled,
        child: _buildMenuTile(
          context,
          labelKey,
          rulerMode == _localRulerMode,
          () {
            AppHaptics.selection();
            setState(() => _localRulerMode = rulerMode);
            widget.onRulerModeSelect(rulerMode);
          },
        ),
      ),
    );
  }

  Widget _buildLayerItem(JourneyKind kind, String labelKey) {
    final isSelected = _localKinds.contains(kind);
    return _buildMenuTile(
      context,
      labelKey,
      isSelected,
      () {
        AppHaptics.selection();
        setState(() {
          final next = Set<JourneyKind>.from(_localKinds);
          if (next.contains(kind)) {
            next.remove(kind);
          } else {
            next.add(kind);
          }
          _localKinds = next;
        });
        _layerTimer?.cancel();
        _layerTimer = Timer(const Duration(milliseconds: 600), () {
          _layerTimer = null;
          widget.onJourneyKindsChanged?.call(_localKinds);
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

/// Mode button: square, semi-transparent (matches timeline style); tap opens [CustomPopup] menu.
/// The caption is derived from the current [viewMode] together with [rulerMode],
/// showing the selected date in the format appropriate for the active timeline/ruler configuration.
class TimeRangeControllerBall extends StatelessWidget {
  final TimeMachineViewMode viewMode;
  final TimeRulerMode rulerMode;
  final DateTime selectedDate;
  final bool loading;

  const TimeRangeControllerBall({
    super.key,
    required this.viewMode,
    required this.rulerMode,
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
    final bool isCustom = viewMode == TimeMachineViewMode.custom;

    final String caption = switch (viewMode) {
      TimeMachineViewMode.period => context.tr('time_machine.menu_view_period'),
      TimeMachineViewMode.asOf => context.tr('time_machine.menu_view_as_of'),
      TimeMachineViewMode.custom => '',
    };

    // Only show what the ruler doesn't: day mode -> year-month; month mode -> year; year mode -> year.
    final String mainText = isCustom
        ? context.tr('time_machine.menu_view_custom')
        : switch (rulerMode) {
            TimeRulerMode.year => '$y',
            TimeRulerMode.month => '$y',
            TimeRulerMode.day => '$y-$m',
            TimeRulerMode.any => '$y',
          };
    final content = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (caption.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: 2),
            child: Text(
              caption,
              style: TextStyle(
                color: Colors.white70,
                fontSize: 10,
                fontWeight: FontWeight.w600,
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        Text(mainText, style: _contentStyle),
      ],
    );

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
                maxLines: 1,
                softWrap: false,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.8),
                  fontSize: 10,
                ),
              ),
              const SizedBox(height: 2),
              Text(
                value,
                maxLines: 1,
                softWrap: false,
                overflow: TextOverflow.ellipsis,
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
