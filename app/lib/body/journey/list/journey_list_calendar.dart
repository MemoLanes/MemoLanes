import 'dart:async';
import 'dart:math' as math;

import 'package:memolanes/theme/app_colors.dart';

import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/list/journey_layer_filter_menu.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_calendar_mode_picker.dart';
import 'package:memolanes/common/component/app_menu.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class JourneyListCalendar extends StatefulWidget {
  final JourneyListController controller;
  final SimpleDate firstDate;

  /// Let a scrolling header expand for localized text and large text sizes.
  final bool fitLabels;

  const JourneyListCalendar({
    super.key,
    required this.controller,
    required this.firstDate,
    this.fitLabels = false,
  });

  @override
  State<JourneyListCalendar> createState() => _JourneyListCalendarState();
}

class _JourneyListCalendarState extends State<JourneyListCalendar> {
  static const _compactWidthBreakpoint = 360.0;

  late final DateTime _initialSelectedDate;
  CalendarDatePicker2Mode _calendarViewMode = CalendarDatePicker2Mode.day;
  GlobalKey _pickerKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    _initialSelectedDate = DateUtils.dateOnly(
      widget.controller.selectedDate.toLocalDateTime(),
    );
  }

  @override
  void didUpdateWidget(covariant JourneyListCalendar oldWidget) {
    super.didUpdateWidget(oldWidget);
    // The package's page indices are relative to this origin.
    if (oldWidget.firstDate != widget.firstDate) _pickerKey = GlobalKey();
  }

  void _setCalendarViewMode(CalendarDatePicker2Mode mode) {
    setState(() {
      // Selecting a month returns the package to day mode internally, while
      // the externally held mode may still be month. Recreate on every header
      // tap so the requested mode always takes effect on the first tap.
      _pickerKey = GlobalKey();
      _calendarViewMode = mode;
    });
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < _compactWidthBreakpoint;
        final calendar = _buildCalendar(context, compact: compact);
        if (!widget.fitLabels) return calendar;
        final style = compact
            ? AppTypography.sectionLabel
            : AppTypography.cardTitle;
        double widthOf(String label, TextStyle textStyle) {
          final painter = TextPainter(
            text: TextSpan(
              text: label,
              style: DefaultTextStyle.of(context).style.merge(textStyle),
            ),
            textScaler: MediaQuery.textScalerOf(context),
            textDirection: Directionality.of(context),
          )..layout();
          final width = painter.width;
          painter.dispose();
          return width;
        }

        final locale = Localizations.localeOf(context).toString();
        final monthWidth = [
          for (var month = 1; month <= 12; month++)
            widthOf(
              (compact ? DateFormat.MMM(locale) : DateFormat.MMMM(locale))
                  .format(DateTime(2000, month)),
              style,
            ),
        ].reduce(math.max);
        final yearWidth = widthOf(
          MaterialLocalizations.of(context)
              .formatYear(DateTime(widget.controller.selectedDate.year)),
          style,
        );
        final filterWidth = compact
            ? 24.0
            : widthOf(_filterLabel(context), AppTypography.cardTitle) + 32;
        // Seven compact 30pt days plus the picker's existing 8pt side padding;
        // the header adds two navigation buttons, dropdowns and measured text.
        final minimum = math.max(
          compact ? 30.0 * 7 + 16 : 0.0,
          108 +
              48 +
              16 +
              (compact ? 8 : 16) +
              yearWidth +
              monthWidth +
              filterWidth,
        );
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: SizedBox(
            width: math.max(constraints.maxWidth, minimum),
            child: calendar,
          ),
        );
      },
    );
  }

  Widget _buildCalendar(BuildContext context, {required bool compact}) {
    final controller = widget.controller;
    final controlsTextStyle =
        (compact ? AppTypography.sectionLabel : AppTypography.cardTitle)
            .copyWith(color: context.appColors.deepGreen);
    final config = CalendarDatePicker2Config(
      firstDate: widget.firstDate.toLocalDateTime(),
      lastDate: controller.lastDate.toLocalDateTime(),
      calendarViewMode: _calendarViewMode,
      centerAlignModePicker: true,
      disableMonthPicker: false,
      // AppCalendarModePicker handles the reordered slots and their taps.
      disableModePicker: true,
      semanticsDictionary: yearFirstCalendarModePickerSemantics(context),
      calendarType: CalendarDatePicker2Type.single,
      selectedDayHighlightColor: context.appColors.primaryGreen,
      daySplashColor: Colors.transparent,
      controlsHeight: widget.fitLabels
          ? math.max(
              compact ? 44.0 : 52.0,
              MediaQuery.textScalerOf(context)
                          .scale(controlsTextStyle.fontSize!) *
                      controlsTextStyle.height! +
                  16,
            )
          : compact
          ? 38
          : null,
      dayMaxWidth: compact ? 30 : null,
      dayTextStyle: (compact ? AppTypography.caption : AppTypography.body)
          .copyWith(color: context.appColors.inkColor),
      selectedDayTextStyle:
          (compact ? AppTypography.caption : AppTypography.body).copyWith(
            color: context.appColors.selectedCalendarInk,
          ),
      todayTextStyle: (compact ? AppTypography.caption : AppTypography.body)
          .copyWith(
            color: context.appColors.deepGreen,
            fontWeight: FontWeight.w700,
          ),
      weekdayLabelTextStyle:
          (compact ? AppTypography.micro : AppTypography.label).copyWith(
            color: context.appColors.mutedInkColor,
          ),
      controlsTextStyle: controlsTextStyle,
      modePickersGap: compact ? 4 : 8,
      modePickerBuilder:
          ({required viewMode, required monthDate, isMonthPicker}) {
            final occupiesPackageMonthSlot = isMonthPicker == true;
            return AppCalendarModePicker(
              viewMode: viewMode,
              monthDate: monthDate,
              occupiesPackageMonthSlot: occupiesPackageMonthSlot,
              textStyle: controlsTextStyle,
              abbreviateMonth: compact,
              onModeChanged: (mode) {
                AppHaptics.selection();
                _setCalendarViewMode(mode);
              },
              trailing: occupiesPackageMonthSlot
                  ? null
                  : _buildFilterButton(context, compact: compact),
              trailingGap: compact ? 4 : 8,
            );
          },
      selectableYearPredicate: (year) =>
          controller.yearsWithJourneys.contains(year),
      selectableMonthPredicate: (year, month) =>
          controller.monthsForYear(year).contains(month),
      selectableDayPredicate: (day) =>
          controller.daysForMonth(day.year, day.month).contains(day.day),
      dayBuilder:
          ({
            required date,
            textStyle,
            decoration,
            isSelected,
            isDisabled,
            isToday,
          }) {
            if (!controller
                .daysForMonth(date.year, date.month)
                .contains(date.day)) {
              return null;
            }
            return Container(
              decoration: isSelected == true
                  ? BoxDecoration(
                      color: context.appColors.primaryGreen,
                      shape: BoxShape.circle,
                    )
                  : null,
              child: Center(
                child: Stack(
                  alignment: AlignmentDirectional.center,
                  children: [
                    Text(
                      MaterialLocalizations.of(context).formatDecimal(date.day),
                      style: textStyle,
                    ),
                    Padding(
                      padding: EdgeInsets.only(top: compact ? 20 : 27.5),
                      child: Container(
                        height: compact ? 3 : 4,
                        width: compact ? 3 : 4,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(5),
                          color: context.appColors.journeyYellow,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            );
          },
      dynamicCalendarRows: true,
      disabledDayTextStyle:
          (compact ? AppTypography.caption : AppTypography.body).copyWith(
            color: context.appColors.mutedInkColor.withValues(alpha: 0.5),
          ),
      disabledMonthTextStyle: AppTypography.body.copyWith(
        color: context.appColors.mutedInkColor.withValues(alpha: 0.5),
        fontWeight: FontWeight.w400,
      ),
      disabledYearTextStyle: AppTypography.body.copyWith(
        color: context.appColors.mutedInkColor.withValues(alpha: 0.5),
        fontWeight: FontWeight.w400,
      ),
      monthBuilder:
          ({
            required month,
            textStyle,
            decoration,
            isSelected,
            isDisabled,
            isCurrentMonth,
          }) {
            final selectedDate = controller.selectedDate;
            return AppCalendarGridOption(
              label: DateFormat.MMM(Localizations.localeOf(context).toString())
                  .format(DateTime(selectedDate.year, month)),
              isSelected: month == selectedDate.month,
              isOriginal:
                  selectedDate.year == _initialSelectedDate.year &&
                  month == _initialSelectedDate.month,
              isCurrent: isCurrentMonth == true,
              isDisabled: isDisabled == true,
              textStyle: textStyle,
            );
          },
      yearBuilder:
          ({
            required year,
            textStyle,
            decoration,
            isSelected,
            isDisabled,
            isCurrentYear,
          }) {
            return AppCalendarGridOption(
              label: MaterialLocalizations.of(context)
                  .formatYear(DateTime(year)),
              isSelected: year == controller.selectedDate.year,
              isOriginal: year == _initialSelectedDate.year,
              isCurrent: isCurrentYear == true,
              isDisabled: isDisabled == true,
              textStyle: textStyle,
            );
          },
      disableVibration: true,
    );

    return CalendarDatePicker2(
      // Page indices are relative to firstDate. Recreate the picker when that
      // origin changes so the visible grid still matches the selected month.
      key: _pickerKey,
      config: config,
      displayedMonthDate: controller.selectedDate.toLocalDateTime(),
      value: controller.hasJourneyOnSelectedDate
          ? [controller.selectedDate.toLocalDateTime()]
          : [],
      onValueChanged: (dates) {
        AppHaptics.selection();
        _runWithLoading(
          () => controller.selectDate(dates.first.toSimpleDate()),
        );
      },
      onDisplayedMonthChanged: (value) {
        // CalendarDatePicker2 already returned its internal view to day mode.
        // Avoid an early rebuild here: the controller updates the externally
        // displayed month asynchronously.
        _calendarViewMode = CalendarDatePicker2Mode.day;
        AppHaptics.selection();
        _runWithLoading(() => controller.displayMonth(value.toSimpleDate()));
      },
    );
  }

  Widget _buildFilterButton(BuildContext context, {required bool compact}) {
    final controller = widget.controller;
    final label = _filterLabel(context);
    return AppMenu(
      menuBuilder: (_) => JourneyLayerFilterMenu(
        selectedKinds: controller.selectedJourneyKinds,
        onChanged: (kinds) {
          AppHaptics.selection();
          _runWithLoading(() => controller.setJourneyKinds(kinds));
        },
      ),
      position: AppMenuPosition.bottom,
      offset: const Offset(-8, 8),
      surfaceBuilder: (context, child) => AppMenuSurface(
        radius: 12,
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: child,
      ),
      child: PointerInterceptor(
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: compact ? 2 : 4,
            vertical: compact ? 6 : 8,
          ),
          child: compact
              ? Tooltip(
                  message: label,
                  child: Icon(
                    Icons.layers_outlined,
                    color: context.appColors.deepGreen,
                    size: 20,
                  ),
                )
              : Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      label,
                      style: AppTypography.cardTitle.copyWith(
                        color: context.appColors.deepGreen,
                      ),
                    ),
                    Icon(
                      Icons.arrow_drop_down,
                      color: context.appColors.deepGreen,
                    ),
                  ],
                ),
        ),
      ),
    );
  }

  String _filterLabel(BuildContext context) {
    final controller = widget.controller;
    if (controller.selectedJourneyKinds.length == 2) {
      return context.tr('journey.list.filter_all_layers');
    }
    return controller.selectedJourneyKinds.single == JourneyKind.defaultKind
        ? context.tr('journey_kind.default')
        : context.tr('journey_kind.flight');
  }

  void _runWithLoading(Future<void> Function() task) {
    unawaited(GlobalLoadingManager.instance.runWithLoading(task));
  }
}
