import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_calendar_mode_picker.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// Shows the compact calendar dialog shared by Journey and Time Machine.
Future<DateTime?> showAppDatePickerDialog(
  BuildContext context, {
  required DateTime initialDate,
  required DateTime firstDate,
  required DateTime lastDate,
  bool highlightInitialDate = false,
  double glassBackgroundAlpha = 0.84,
}) {
  assert(!firstDate.isAfter(lastDate));
  assert(!initialDate.isBefore(firstDate));
  assert(!initialDate.isAfter(lastDate));
  assert(glassBackgroundAlpha >= 0 && glassBackgroundAlpha <= 1);

  return showDialog<DateTime>(
    context: context,
    barrierColor: StyleConstants.shadowColor.withValues(
      alpha: StyleConstants.isDarkMode ? 0.58 : 0.2,
    ),
    builder: (_) => PointerInterceptor(
      child: _AppDatePickerDialog(
        initialDate: initialDate,
        firstDate: firstDate,
        lastDate: lastDate,
        highlightInitialDate: highlightInitialDate,
        glassBackgroundAlpha: glassBackgroundAlpha,
      ),
    ),
  );
}

class _AppDatePickerDialog extends StatefulWidget {
  const _AppDatePickerDialog({
    required this.initialDate,
    required this.firstDate,
    required this.lastDate,
    required this.highlightInitialDate,
    required this.glassBackgroundAlpha,
  });

  final DateTime initialDate;
  final DateTime firstDate;
  final DateTime lastDate;
  final bool highlightInitialDate;
  final double glassBackgroundAlpha;

  @override
  State<_AppDatePickerDialog> createState() => _AppDatePickerDialogState();
}

class _AppDatePickerDialogState extends State<_AppDatePickerDialog> {
  late DateTime _selectedDate;
  late DateTime _displayedMonthDate;
  CalendarDatePicker2Mode _calendarViewMode = CalendarDatePicker2Mode.day;
  int _calendarPickerRevision = 0;

  @override
  void initState() {
    super.initState();
    _selectedDate = widget.initialDate;
    _displayedMonthDate = DateTime(
      widget.initialDate.year,
      widget.initialDate.month,
    );
  }

  void _setCalendarViewMode(CalendarDatePicker2Mode mode) {
    setState(() {
      // calendar_date_picker2 returns to day mode internally after a month is
      // selected, but it does not emit onDisplayedMonthChanged when the user
      // selects the already displayed month. Recreate only in that stale-state
      // case so the same month selector can always be opened again.
      if (_calendarViewMode == mode) {
        _calendarPickerRevision += 1;
      }
      _calendarViewMode = mode;
    });
  }

  Widget _buildDay({
    required DateTime date,
    required TextStyle? textStyle,
    required bool? isSelected,
  }) {
    final selected = isSelected == true;
    final showInitialDate =
        widget.highlightInitialDate &&
        DateUtils.isSameDay(date, widget.initialDate);

    return Center(
      child: Container(
        width: 28,
        height: 28,
        alignment: Alignment.center,
        decoration: selected
            ? BoxDecoration(
                shape: BoxShape.circle,
                color: StyleConstants.primaryGreen,
              )
            : showInitialDate
            ? BoxDecoration(
                shape: BoxShape.circle,
                border: Border.all(
                  color: StyleConstants.primaryGreen,
                  width: 2,
                ),
              )
            : null,
        child: Text(
          MaterialLocalizations.of(context).formatDecimal(date.day),
          style: selected
              ? AppTypography.caption.copyWith(
                  color: StyleConstants.isDarkMode
                      ? StyleConstants.onPrimaryActionColor
                      : StyleConstants.inkColor,
                )
              : textStyle,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final useCompactSpacing = MediaQuery.sizeOf(context).width < 380;
    final localizations = MaterialLocalizations.of(context);
    final controlsTextStyle = AppTypography.sectionLabel.copyWith(
      color: StyleConstants.deepGreen,
    );
    final config = CalendarDatePicker2Config(
      firstDate: widget.firstDate,
      lastDate: widget.lastDate,
      calendarType: CalendarDatePicker2Type.single,
      calendarViewMode: _calendarViewMode,
      centerAlignModePicker: true,
      disableMonthPicker: false,
      // The shared selector owns these taps so the package's fixed physical
      // month/year slots cannot open the opposite view after being reordered.
      disableModePicker: true,
      semanticsDictionary: yearFirstCalendarModePickerSemantics(context),
      modePickerBuilder:
          ({required viewMode, required monthDate, isMonthPicker}) {
            return AppCalendarModePicker(
              viewMode: viewMode,
              monthDate: monthDate,
              occupiesPackageMonthSlot: isMonthPicker == true,
              textStyle: controlsTextStyle,
              onModeChanged: (mode) {
                AppHaptics.selection();
                _setCalendarViewMode(mode);
              },
            );
          },
      controlsHeight: 38,
      dayMaxWidth: 30,
      dynamicCalendarRows: true,
      disableVibration: true,
      daySplashColor: Colors.transparent,
      selectedDayHighlightColor: StyleConstants.primaryGreen,
      dayTextStyle: AppTypography.caption.copyWith(
        color: StyleConstants.inkColor,
      ),
      selectedDayTextStyle: AppTypography.caption.copyWith(
        color: StyleConstants.isDarkMode
            ? StyleConstants.onPrimaryActionColor
            : StyleConstants.inkColor,
      ),
      todayTextStyle: AppTypography.caption.copyWith(
        color: StyleConstants.deepGreen,
        fontWeight: FontWeight.w700,
      ),
      monthTextStyle: AppTypography.body.copyWith(
        color: StyleConstants.inkColor,
      ),
      selectedMonthTextStyle: AppTypography.body.copyWith(
        color: StyleConstants.isDarkMode
            ? StyleConstants.onPrimaryActionColor
            : StyleConstants.inkColor,
        fontWeight: FontWeight.w600,
      ),
      yearTextStyle: AppTypography.body.copyWith(
        color: StyleConstants.inkColor,
      ),
      selectedYearTextStyle: AppTypography.body.copyWith(
        color: StyleConstants.isDarkMode
            ? StyleConstants.onPrimaryActionColor
            : StyleConstants.inkColor,
        fontWeight: FontWeight.w600,
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
            final displayedYear = _displayedMonthDate.year;
            return AppCalendarGridOption(
              label: DateFormat.MMM(Localizations.localeOf(context).toString())
                  .format(DateTime(displayedYear, month)),
              isSelected: month == _displayedMonthDate.month,
              isOriginal:
                  displayedYear == widget.initialDate.year &&
                  month == widget.initialDate.month,
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
              isSelected: year == _displayedMonthDate.year,
              isOriginal: year == widget.initialDate.year,
              isCurrent: isCurrentYear == true,
              isDisabled: isDisabled == true,
              textStyle: textStyle,
            );
          },
      weekdayLabelTextStyle: AppTypography.micro.copyWith(
        color: StyleConstants.mutedInkColor,
      ),
      controlsTextStyle: controlsTextStyle,
      disabledDayTextStyle: AppTypography.caption.copyWith(
        color: StyleConstants.mutedInkColor.withValues(alpha: 0.42),
      ),
      lastMonthIcon: Icon(
        Icons.chevron_left_rounded,
        color: StyleConstants.deepGreen,
        size: 20,
      ),
      nextMonthIcon: Icon(
        Icons.chevron_right_rounded,
        color: StyleConstants.deepGreen,
        size: 20,
      ),
      dayBuilder:
          ({
            required date,
            textStyle,
            decoration,
            isSelected,
            isDisabled,
            isToday,
          }) {
            return _buildDay(
              date: date,
              textStyle: textStyle,
              isSelected: isSelected,
            );
          },
    );

    return Dialog(
      elevation: 0,
      backgroundColor: Colors.transparent,
      insetPadding: EdgeInsets.symmetric(
        horizontal: useCompactSpacing ? 8 : 24,
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 390),
        child: AppDialogSurface(
          style: AppDialogSurfaceStyle.glass,
          glassBackgroundAlpha: widget.glassBackgroundAlpha,
          child: Padding(
            padding: EdgeInsets.fromLTRB(
              useCompactSpacing ? 4 : 12,
              10,
              useCompactSpacing ? 4 : 12,
              12,
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                SizedBox(
                  width: double.infinity,
                  height: 290,
                  child: CalendarDatePicker2(
                    key: ValueKey('app-date-picker-$_calendarPickerRevision'),
                    config: config,
                    displayedMonthDate: _displayedMonthDate,
                    value: [_selectedDate],
                    onValueChanged: (dates) {
                      final selected = dates.firstOrNull;
                      if (selected == null) return;
                      AppHaptics.selection();
                      setState(() => _selectedDate = selected);
                    },
                    onDisplayedMonthChanged: (displayedMonth) {
                      // The package has already returned its own view to day
                      // mode. Keep our next config rebuild in sync without
                      // rebuilding early and resetting its newly chosen month.
                      _displayedMonthDate = DateTime(
                        displayedMonth.year,
                        displayedMonth.month,
                      );
                      _calendarViewMode = CalendarDatePicker2Mode.day;
                    },
                  ),
                ),
                const SizedBox(height: 6),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    SizedBox(
                      width: 112,
                      child: AppButton(
                        label: localizations.cancelButtonLabel,
                        icon: Icons.close_rounded,
                        variant: AppButtonVariant.secondary,
                        size: AppButtonSize.compact,
                        expand: true,
                        onPressed: () => Navigator.of(context).pop(),
                      ),
                    ),
                    const SizedBox(width: 8),
                    SizedBox(
                      width: 112,
                      child: AppButton(
                        label: localizations.okButtonLabel,
                        icon: Icons.check_rounded,
                        variant: AppButtonVariant.primary,
                        size: AppButtonSize.compact,
                        expand: true,
                        onPressed: () =>
                            Navigator.of(context).pop(_selectedDate),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
