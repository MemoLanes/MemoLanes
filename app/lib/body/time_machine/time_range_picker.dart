import 'dart:math' as math;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/time_machine/time_machine_glass_surface.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_date_picker_dialog.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'time_ruler.dart';

import 'package:memolanes/src/rust/journey_header.dart';

export 'time_ruler.dart' show TimeRulerMode, TimeRuler;

/// Time range picker with mode-based range selection.
/// Supports the [TimeMachineViewMode.period],
/// [TimeMachineViewMode.asOf], and [TimeMachineViewMode.custom] views,
/// with [TimeRulerMode] controlling the time granularity shown by the ruler.
/// Reports range changes via [onRangeChanged]. A committed interaction (for
/// example, a ruler selection after the user releases and snapping completes)
/// uses [onRangeCommitted] when provided.
class TimeRangePicker extends StatefulWidget {
  final SimpleDate? earliestDate;
  final bool loading;
  final void Function(SimpleDate from, SimpleDate to) onRangeChanged;

  /// Optional fast path for a value committed by a user interaction.
  final void Function(SimpleDate from, SimpleDate to)? onRangeCommitted;
  final Set<JourneyKind> selectedJourneyKinds;
  final void Function(Set<JourneyKind>)? onJourneyKindsChanged;

  const TimeRangePicker({
    super.key,
    this.earliestDate,
    this.loading = false,
    required this.onRangeChanged,
    this.onRangeCommitted,
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
  TimeMachineViewMode _viewMode = TimeMachineViewMode.asOf;
  TimeRulerMode _rulerMode = TimeRulerMode.year;
  int _selectedYear = DateTime.now().year;
  int _selectedMonth = DateTime.now().month;
  int _selectedDay = DateTime.now().day;

  /// Only for button display; updates in real time while scrolling; syncs with selected on release.
  int _displayYear = DateTime.now().year;
  int _displayMonth = DateTime.now().month;
  int _displayDay = DateTime.now().day;
  SimpleDate _fromDate = SimpleDate.today();
  SimpleDate _toDate = SimpleDate.today();

  final OverlayPortalController _modeMenuController = OverlayPortalController(
    debugLabel: 'time-machine-mode-menu',
  );
  final LayerLink _modeMenuLayerLink = LayerLink();
  final GlobalKey _modeMenuTargetKey = GlobalKey();
  final Object _modeMenuTapRegionGroup = Object();

  /// Single source of truth for lower bound used by ruler/range/pickers.
  SimpleDate get _effectiveEarliest {
    final now = SimpleDate.today();
    final fallback = SimpleDate(now.year - 1);
    final candidate = widget.earliestDate ?? fallback;
    // Guard: if upstream provides a future earliest, cap to "now".
    return candidate.isAfter(now) ? now : candidate;
  }

  (SimpleDate from, SimpleDate to) _periodRangeForSelection() {
    switch (_rulerMode) {
      case TimeRulerMode.year:
        return (SimpleDate(_selectedYear), SimpleDate(_selectedYear, 12, 31));
      case TimeRulerMode.month:
        return (
          SimpleDate(_selectedYear, _selectedMonth),
          SimpleDate(_selectedYear, _selectedMonth + 1, 0),
        );
      case TimeRulerMode.day:
        final lastDay = SimpleDate(_selectedYear, _selectedMonth + 1, 0).day;
        final d = _selectedDay.clamp(1, lastDay);
        final date = SimpleDate(_selectedYear, _selectedMonth, d);
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

  void _notifyRange({bool committed = false}) {
    final callback = committed && widget.onRangeCommitted != null
        ? widget.onRangeCommitted!
        : widget.onRangeChanged;
    callback(_fromDate, _toDate);
  }

  void _onRulerModeSelected(TimeRulerMode rulerMode) {
    if (rulerMode == _rulerMode) return;
    AppHaptics.selection();
    setState(() {
      _rulerMode = rulerMode;
      _applyCurrentRange();
    });
    _notifyRange(committed: true);
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

  void _toggleModeMenu() {
    _modeMenuController.toggle();
  }

  void _hideModeMenu() {
    if (_modeMenuController.isShowing) {
      _modeMenuController.hide();
    }
  }

  SimpleDate get _displayDate =>
      SimpleDate(_displayYear, _displayMonth, _displayDay);

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
    });
    _notifyRange(committed: true);
  }

  @override
  void initState() {
    super.initState();
    _applyCurrentRange();
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => _notifyRange(committed: true),
    );
  }

  @override
  Widget build(BuildContext context) {
    final rulerChild = _viewMode != TimeMachineViewMode.custom
        ? TimeRuler(
            rulerMode: _rulerMode,
            selectedYear: _selectedYear,
            selectedMonth: _selectedMonth,
            selectedDay: _selectedDay,
            earliest: _effectiveEarliest.toLocalDateTime(),
            onSelectionChanged: (selection) => _commitRulerChange(() {
              _selectedYear = selection.$1;
              if (selection.$2 != null) _selectedMonth = selection.$2!;
              if (selection.$3 != null) _selectedDay = selection.$3!;
            }),
            onDisplayChanged: (s) =>
                setState(() => _updateDisplay(s.$1, s.$2, s.$3)),
          )
        : TimeRangeOverlayPicker(
            fromDate: _fromDate.toLocalDateTime(),
            toDate: _toDate.toLocalDateTime(),
            earliest: _effectiveEarliest.toLocalDateTime(),
            onFromChanged: (d) {
              setState(() {
                _fromDate = d.toSimpleDate();
                if (_fromDate.isBefore(_effectiveEarliest)) {
                  _fromDate = _effectiveEarliest;
                }
                if (_toDate.isBefore(_fromDate)) _toDate = _fromDate;
              });
              _notifyRange();
            },
            onToChanged: (d) {
              setState(() {
                _toDate = d.toSimpleDate();
                if (_toDate.isBefore(_effectiveEarliest)) {
                  _toDate = _effectiveEarliest;
                }
                if (_fromDate.isAfter(_toDate)) _fromDate = _toDate;
              });
              _notifyRange();
            },
          );

    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        OverlayPortal(
          controller: _modeMenuController,
          overlayChildBuilder: (overlayContext) {
            final mediaQuery = MediaQuery.of(overlayContext);
            const menuGap = 12.0;
            const safeAreaMargin = 12.0;
            final maxMenuWidth = math.max(
              0.0,
              mediaQuery.size.width -
                  mediaQuery.viewPadding.left -
                  mediaQuery.viewPadding.right -
                  48,
            );
            final targetRenderObject = _modeMenuTargetKey.currentContext
                ?.findRenderObject();
            final targetBox = targetRenderObject is RenderBox
                ? targetRenderObject
                : null;
            final targetTop = targetBox?.localToGlobal(Offset.zero).dy ?? 0;
            final targetBottom = targetTop + (targetBox?.size.height ?? 0);
            final safeTop = mediaQuery.viewPadding.top + safeAreaMargin;
            final safeBottom =
                mediaQuery.size.height -
                mediaQuery.viewPadding.bottom -
                safeAreaMargin;
            final spaceAbove = math.max(0.0, targetTop - menuGap - safeTop);
            final spaceBelow = math.max(
              0.0,
              safeBottom - targetBottom - menuGap,
            );
            final placeAbove = spaceAbove >= spaceBelow;
            final maxMenuHeight = placeAbove ? spaceAbove : spaceBelow;

            // OverlayPortal's child receives full-screen constraints. Align
            // loosens them before the follower is measured, so the follower's
            // anchor uses the menu's actual size instead of the screen size.
            return Align(
              alignment: Alignment.topLeft,
              child: CompositedTransformFollower(
                link: _modeMenuLayerLink,
                showWhenUnlinked: false,
                targetAnchor: placeAbove
                    ? Alignment.topLeft
                    : Alignment.bottomLeft,
                followerAnchor: placeAbove
                    ? Alignment.bottomLeft
                    : Alignment.topLeft,
                offset: Offset(0, placeAbove ? -menuGap : menuGap),
                child: TapRegion(
                  groupId: _modeMenuTapRegionGroup,
                  onTapOutside: (_) => _hideModeMenu(),
                  child: ConstrainedBox(
                    constraints: BoxConstraints(
                      maxWidth: maxMenuWidth,
                      maxHeight: maxMenuHeight,
                    ),
                    child: TweenAnimationBuilder<double>(
                      tween: Tween(begin: 0, end: 1),
                      duration: const Duration(milliseconds: 160),
                      curve: Curves.easeOutCubic,
                      builder: (context, value, child) => Opacity(
                        opacity: value,
                        child: Transform.scale(
                          scale: 0.96 + value * 0.04,
                          alignment: placeAbove
                              ? Alignment.bottomLeft
                              : Alignment.topLeft,
                          child: child,
                        ),
                      ),
                      child: _buildModeMenu(),
                    ),
                  ),
                ),
              ),
            );
          },
          child: CompositedTransformTarget(
            key: _modeMenuTargetKey,
            link: _modeMenuLayerLink,
            child: TapRegion(
              groupId: _modeMenuTapRegionGroup,
              child: PointerInterceptor(
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTap: _toggleModeMenu,
                  child: TimeRangeControllerBall(
                    key: ValueKey(
                      'ball-$_displayYear-$_displayMonth-$_displayDay',
                    ),
                    viewMode: _viewMode,
                    rulerMode: _rulerMode,
                    selectedDate: _viewMode == TimeMachineViewMode.custom
                        ? _toDate
                        : _displayDate,
                    loading: widget.loading,
                  ),
                ),
              ),
            ),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: TapRegion(
            groupId: _modeMenuTapRegionGroup,
            child: PointerInterceptor(
              child: SizedBox(height: _kPickerBlockHeight, child: rulerChild),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildModeMenu() {
    return TimeMachineGlassSurface(
      padding: const EdgeInsets.all(16),
      child: PointerInterceptor(
        child: SingleChildScrollView(
          child: _TimeMachineViewModeAndLayerMenu(
            currentViewMode: _viewMode,
            onViewModeSelect: _onViewModeSelected,
            currentRulerMode: _rulerMode,
            onRulerModeSelect: _onRulerModeSelected,
            selectedJourneyKinds: widget.selectedJourneyKinds,
            onJourneyKindsChanged: widget.onJourneyKindsChanged,
          ),
        ),
      ),
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
  static const double _minColumnWidth = 96;
  static const double _maxColumnWidth = 116;
  static const double _dividerWidth = 1;

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
    return LayoutBuilder(
      builder: (context, constraints) {
        final availableWidth = constraints.hasBoundedWidth
            ? constraints.maxWidth
            : _maxColumnWidth * 3 + _dividerWidth * 2;
        final columnWidth = ((availableWidth - _dividerWidth * 2) / 3).clamp(
          _minColumnWidth,
          _maxColumnWidth,
        );

        final content = IntrinsicHeight(
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _buildMenuColumn(
                width: columnWidth,
                title: context.tr('time_machine.menu_title_view'),
                children: _viewModeKeys
                    .map((e) => _buildViewModeItem(e.$1, e.$2))
                    .toList(),
              ),
              _buildColumnDivider(),
              _buildMenuColumn(
                width: columnWidth,
                title: context.tr('time_machine.menu_title_granularity'),
                children: _granularityKeys
                    .map((e) => _buildGranularityItem(e.$1, e.$2))
                    .toList(),
              ),
              _buildColumnDivider(),
              _buildMenuColumn(
                width: columnWidth,
                title: context.tr('time_machine.menu_title_layer'),
                children: _layerKeys
                    .map((e) => _buildLayerItem(e.$1, e.$2))
                    .toList(),
              ),
            ],
          ),
        );

        // Keep the three-column grid intact on very small screens instead of
        // compressing labels or letting selected backgrounds touch dividers.
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: content,
        );
      },
    );
  }

  Widget _buildMenuColumn({
    required double width,
    required String title,
    required List<Widget> children,
  }) {
    return SizedBox(
      width: width,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [_buildColumnTitle(title), ...children],
      ),
    );
  }

  Widget _buildColumnDivider() {
    return VerticalDivider(
      width: _dividerWidth,
      thickness: _dividerWidth,
      color: StyleConstants.lineColor,
      indent: 8,
      endIndent: 8,
    );
  }

  Widget _buildColumnTitle(String text) {
    return SizedBox(
      height: 32,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6),
        child: Center(
          child: Text(
            text,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.center,
            style: AppTypography.sectionLabel.copyWith(
              color: StyleConstants.mutedInkColor,
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildMenuTile(
    BuildContext context,
    String labelKey,
    bool isSelected,
    VoidCallback onTap,
  ) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      child: Material(
        color: isSelected
            ? StyleConstants.softGreen.withValues(alpha: 0.62)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(9),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(9),
          child: SizedBox(
            height: 40,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: Row(
                children: [
                  if (isSelected)
                    Icon(
                      Icons.check_rounded,
                      size: 18,
                      color: StyleConstants.deepGreen,
                    )
                  else
                    const SizedBox(width: 18, height: 18),
                  const SizedBox(width: 7),
                  Expanded(
                    child: Text(
                      context.tr(labelKey),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AppTypography.itemTitle.copyWith(
                        color: isSelected
                            ? StyleConstants.deepGreen
                            : StyleConstants.deepGreen.withValues(alpha: 0.82),
                        fontWeight: isSelected
                            ? FontWeight.w700
                            : FontWeight.w600,
                      ),
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

  Widget _buildViewModeItem(TimeMachineViewMode mode, String labelKey) {
    return _buildMenuTile(context, labelKey, mode == _localViewMode, () {
      setState(() => _localViewMode = mode);
      widget.onViewModeSelect(mode);
    });
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
            setState(() => _localRulerMode = rulerMode);
            widget.onRulerModeSelect(rulerMode);
          },
        ),
      ),
    );
  }

  Widget _buildLayerItem(JourneyKind kind, String labelKey) {
    final isSelected = _localKinds.contains(kind);
    return _buildMenuTile(context, labelKey, isSelected, () {
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
      widget.onJourneyKindsChanged?.call(_localKinds);
    });
  }
}

/// Mode button: square, semi-transparent (matches timeline style); tap opens
/// the non-modal mode menu anchored above it.
/// The caption is derived from the current [viewMode] together with [rulerMode],
/// showing the selected date in the format appropriate for the active timeline/ruler configuration.
class TimeRangeControllerBall extends StatelessWidget {
  final TimeMachineViewMode viewMode;
  final TimeRulerMode rulerMode;
  final SimpleDate selectedDate;
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
  TextStyle get _contentStyle => AppTypography.sectionLabel.copyWith(
    color: StyleConstants.deepGreen,
    fontWeight: FontWeight.w700,
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
              style: AppTypography.micro.copyWith(
                color: StyleConstants.mutedInkColor,
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        Text(mainText, style: _contentStyle),
      ],
    );

    return TimeMachineGlassSurface(
      borderRadius: BorderRadius.circular(_borderRadius),
      child: SizedBox(
        width: _buttonSize,
        height: _buttonSize,
        child: Center(
          child: FittedBox(
            fit: BoxFit.scaleDown,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 6),
              child: content,
            ),
          ),
        ),
      ),
    );
  }
}

/// Fixed height for ruler / any picker block; must fit both ruler and any-mode date tiles.
const double _kPickerBlockHeight = 60.0;

Widget _buildGlassPanel(Widget child, {EdgeInsets? padding}) {
  return TimeMachineGlassSurface(padding: padding, child: child);
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

  static String _compactDate(DateTime date) {
    final month = date.month.toString().padLeft(2, '0');
    final day = date.day.toString().padLeft(2, '0');
    return '${date.year}\n$month-$day';
  }

  static double _singleLineWidth(
    BuildContext context,
    String text,
    TextStyle style,
  ) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      maxLines: 1,
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
    )..layout();
    final width = painter.width;
    painter.dispose();
    return width;
  }

  @override
  Widget build(BuildContext context) {
    final fromValue = _fmt.format(fromDate);
    final toValue = _fmt.format(toDate);
    final valueStyle = AppTypography.label.copyWith(
      color: StyleConstants.deepGreen,
    );
    final widestValue = math.max(
      _singleLineWidth(context, fromValue, valueStyle),
      _singleLineWidth(context, toValue, valueStyle),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        const regularOuterPadding = 16.0;
        const regularPanelPadding = 16.0;
        const regularTilePadding = 10.0;
        const regularGap = 12.0;
        final regularRequiredWidth =
            regularOuterPadding * 2 +
            regularPanelPadding * 2 +
            regularGap +
            (widestValue + regularTilePadding * 2) * 2;
        final compact =
            constraints.hasBoundedWidth &&
            constraints.maxWidth < regularRequiredWidth;
        final outerPadding = compact ? 4.0 : regularOuterPadding;
        final panelPadding = compact ? 6.0 : regularPanelPadding;
        final gap = compact ? 6.0 : regularGap;

        return Padding(
          padding: EdgeInsets.symmetric(horizontal: outerPadding),
          child: _buildGlassPanel(
            Row(
              children: [
                Expanded(
                  child: _TapTile(
                    label: context.tr('journey.start_time'),
                    value: fromValue,
                    compactValue: _compactDate(fromDate),
                    compactSpacing: compact,
                    onTap: () => _showDatePicker(
                      context,
                      fromDate,
                      earliest,
                      onFromChanged,
                    ),
                  ),
                ),
                SizedBox(width: gap),
                Expanded(
                  child: _TapTile(
                    label: context.tr('journey.end_time'),
                    value: toValue,
                    compactValue: _compactDate(toDate),
                    compactSpacing: compact,
                    onTap: () =>
                        _showDatePicker(context, toDate, earliest, onToChanged),
                  ),
                ),
              ],
            ),
            padding: EdgeInsets.symmetric(
              horizontal: panelPadding,
              vertical: compact ? 3 : 6,
            ),
          ),
        );
      },
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
    final picked = await showAppDatePickerDialog(
      context,
      initialDate: safeInitial,
      firstDate: first,
      lastDate: last,
      highlightInitialDate: true,
    );
    if (picked != null) onChanged(picked);
  }
}

class _TapTile extends StatelessWidget {
  final String label;
  final String value;
  final String compactValue;
  final bool compactSpacing;
  final VoidCallback onTap;

  const _TapTile({
    required this.label,
    required this.value,
    required this.compactValue,
    required this.compactSpacing,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final horizontalPadding = compactSpacing ? 4.0 : 10.0;
        final valueStyle = AppTypography.label.copyWith(
          color: StyleConstants.deepGreen,
          fontSize: compactSpacing ? 11 : null,
        );
        final valueWidth = TimeRangeOverlayPicker._singleLineWidth(
          context,
          value,
          valueStyle,
        );
        final availableValueWidth = constraints.hasBoundedWidth
            ? math.max(0.0, constraints.maxWidth - horizontalPadding * 2)
            : double.infinity;
        final splitValue = valueWidth > availableValueWidth;

        return Material(
          color: Colors.transparent,
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(8),
            child: Padding(
              padding: EdgeInsets.symmetric(
                horizontal: horizontalPadding,
                vertical: splitValue ? 2 : 4,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    label,
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                    style: AppTypography.micro.copyWith(
                      color: StyleConstants.mutedInkColor,
                    ),
                  ),
                  SizedBox(height: splitValue ? 0 : 2),
                  Text(
                    splitValue ? compactValue : value,
                    maxLines: splitValue ? 2 : 1,
                    softWrap: false,
                    style: valueStyle,
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}
