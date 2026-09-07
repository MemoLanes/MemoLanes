import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

/// A calendar mode selector that compensates for calendar_date_picker2's
/// fixed month-first slot order while keeping the matching tap targets.
class AppCalendarModePicker extends StatelessWidget {
  const AppCalendarModePicker({
    super.key,
    required this.viewMode,
    required this.monthDate,
    required this.occupiesPackageMonthSlot,
    required this.textStyle,
    required this.onModeChanged,
    this.trailing,
    this.trailingGap = 8,
  });

  final CalendarDatePicker2Mode viewMode;
  final DateTime monthDate;

  /// The package always places this slot first. We deliberately show the year
  /// here, then use the package's year slot for the month.
  final bool occupiesPackageMonthSlot;
  final TextStyle textStyle;
  final ValueChanged<CalendarDatePicker2Mode> onModeChanged;
  final Widget? trailing;
  final double trailingGap;

  @override
  Widget build(BuildContext context) {
    final targetMode = occupiesPackageMonthSlot
        ? CalendarDatePicker2Mode.year
        : CalendarDatePicker2Mode.month;
    final isActive = viewMode == targetMode;
    final label = targetMode == CalendarDatePicker2Mode.year
        ? MaterialLocalizations.of(context).formatYear(monthDate)
        : DateFormat.MMMM(Localizations.localeOf(context).toString())
              .format(monthDate);

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: () => onModeChanged(
            isActive ? CalendarDatePicker2Mode.day : targetMode,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: textStyle,
                ),
                AnimatedRotation(
                  turns: isActive ? 0.5 : 0,
                  duration: const Duration(milliseconds: 200),
                  curve: Curves.easeOut,
                  child: Icon(
                    Icons.arrow_drop_down,
                    color: StyleConstants.deepGreen,
                  ),
                ),
              ],
            ),
          ),
        ),
        if (trailing != null) ...[SizedBox(width: trailingGap), trailing!],
      ],
    );
  }
}

/// Shared visual states for the month and year grids:
/// current period = deep-green text, original value = outlined,
/// actively displayed value = filled.
class AppCalendarGridOption extends StatelessWidget {
  const AppCalendarGridOption({
    super.key,
    required this.label,
    required this.isSelected,
    required this.isOriginal,
    required this.isCurrent,
    required this.isDisabled,
    this.textStyle,
  });

  final String label;
  final bool isSelected;
  final bool isOriginal;
  final bool isCurrent;
  final bool isDisabled;
  final TextStyle? textStyle;

  @override
  Widget build(BuildContext context) {
    final foregroundColor = isDisabled
        ? StyleConstants.mutedInkColor.withValues(alpha: 0.42)
        : isSelected
        ? (StyleConstants.isDarkMode
              ? StyleConstants.onPrimaryActionColor
              : StyleConstants.inkColor)
        : isCurrent
        ? StyleConstants.deepGreen
        : StyleConstants.inkColor;
    final decoration = isSelected
        ? BoxDecoration(
            color: StyleConstants.primaryGreen,
            borderRadius: BorderRadius.circular(18),
          )
        : isOriginal
        ? BoxDecoration(
            border: Border.all(color: StyleConstants.primaryGreen, width: 2),
            borderRadius: BorderRadius.circular(18),
          )
        : null;

    return Center(
      child: Container(
        width: 72,
        height: 36,
        alignment: Alignment.center,
        decoration: decoration,
        child: Semantics(
          selected: isSelected,
          child: Text(
            label,
            style: (textStyle ?? AppTypography.body).copyWith(
              color: foregroundColor,
              fontWeight: isCurrent
                  ? FontWeight.w700
                  : isSelected
                  ? FontWeight.w600
                  : FontWeight.w400,
            ),
          ),
        ),
      ),
    );
  }
}

/// calendar_date_picker2 assigns semantics to its physical slots. Since the
/// visual and interactive contents are reversed, the slot labels must follow.
Map<CalendarDatePicker2SemanticsLabel, String?>
yearFirstCalendarModePickerSemantics(BuildContext context) => {
  CalendarDatePicker2SemanticsLabel.selectMonth: context.tr(
    'common.select_year',
  ),
  CalendarDatePicker2SemanticsLabel.selectYear: context.tr(
    'common.select_month',
  ),
};
