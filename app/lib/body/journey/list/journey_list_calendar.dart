import 'dart:async';

import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/list/journey_layer_filter_menu.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_calendar_mode_picker.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class JourneyListCalendar extends StatefulWidget {
  final JourneyListController controller;
  final SimpleDate firstDate;
  final bool compact;

  const JourneyListCalendar({
    super.key,
    required this.controller,
    required this.firstDate,
    this.compact = false,
  });

  @override
  State<JourneyListCalendar> createState() => _JourneyListCalendarState();
}

class _JourneyListCalendarState extends State<JourneyListCalendar> {
  late final DateTime _initialSelectedDate;
  CalendarDatePicker2Mode _calendarViewMode = CalendarDatePicker2Mode.day;
  int _calendarPickerRevision = 0;

  @override
  void initState() {
    super.initState();
    _initialSelectedDate = DateUtils.dateOnly(
      widget.controller.selectedDate.toLocalDateTime(),
    );
  }

  void _setCalendarViewMode(CalendarDatePicker2Mode mode) {
    setState(() {
      // Force a fresh picker only when the package's internal day mode and our
      // externally stored mode became out of sync after reselecting a month.
      if (_calendarViewMode == mode) {
        _calendarPickerRevision += 1;
      }
      _calendarViewMode = mode;
    });
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final compact = widget.compact;
    final controlsTextStyle =
        (compact ? AppTypography.sectionLabel : AppTypography.cardTitle)
            .copyWith(color: StyleConstants.deepGreen);
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
      selectedDayHighlightColor: StyleConstants.primaryGreen,
      controlsHeight: compact ? 38 : null,
      dayMaxWidth: compact ? 30 : null,
      dayTextStyle: (compact ? AppTypography.caption : AppTypography.body)
          .copyWith(color: StyleConstants.inkColor),
      selectedDayTextStyle:
          (compact ? AppTypography.caption : AppTypography.body).copyWith(
            color: StyleConstants.isDarkMode
                ? StyleConstants.onPrimaryActionColor
                : StyleConstants.inkColor,
          ),
      todayTextStyle: (compact ? AppTypography.caption : AppTypography.body)
          .copyWith(
            color: StyleConstants.deepGreen,
            fontWeight: FontWeight.w700,
          ),
      weekdayLabelTextStyle:
          (compact ? AppTypography.micro : AppTypography.label).copyWith(
            color: StyleConstants.mutedInkColor,
          ),
      controlsTextStyle: controlsTextStyle,
      modePickersGap: 8,
      modePickerBuilder:
          ({required viewMode, required monthDate, isMonthPicker}) {
            final occupiesPackageMonthSlot = isMonthPicker == true;
            return AppCalendarModePicker(
              viewMode: viewMode,
              monthDate: monthDate,
              occupiesPackageMonthSlot: occupiesPackageMonthSlot,
              textStyle: controlsTextStyle,
              onModeChanged: (mode) {
                AppHaptics.selection();
                _setCalendarViewMode(mode);
              },
              trailing: occupiesPackageMonthSlot
                  ? null
                  : _buildFilterButton(context),
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
                      color: StyleConstants.primaryGreen,
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
                          color: StyleConstants.journeyYellow,
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
            color: StyleConstants.mutedInkColor.withValues(alpha: 0.5),
          ),
      disabledMonthTextStyle: AppTypography.body.copyWith(
        color: StyleConstants.mutedInkColor.withValues(alpha: 0.5),
        fontWeight: FontWeight.w400,
      ),
      disabledYearTextStyle: AppTypography.body.copyWith(
        color: StyleConstants.mutedInkColor.withValues(alpha: 0.5),
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
      key: ValueKey('journey-list-calendar-$_calendarPickerRevision'),
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

  Widget _buildFilterButton(BuildContext context) {
    final controller = widget.controller;
    final label = _filterLabel(context);
    return CustomPopup(
      position: PopupPosition.bottom,
      horizontalOffset: -8,
      verticalOffset: 8,
      contentPadding: const EdgeInsets.symmetric(vertical: 8),
      contentRadius: 12,
      barrierColor: Colors.transparent,
      content: PointerInterceptor(
        child: JourneyLayerFilterMenu(
          selectedKinds: controller.selectedJourneyKinds,
          onChanged: (kinds) {
            AppHaptics.selection();
            _runWithLoading(() => controller.setJourneyKinds(kinds));
          },
        ),
      ),
      child: PointerInterceptor(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                label,
                style:
                    (widget.compact
                            ? AppTypography.sectionLabel
                            : AppTypography.cardTitle)
                        .copyWith(color: StyleConstants.deepGreen),
              ),
              Icon(Icons.arrow_drop_down, color: StyleConstants.deepGreen),
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
