import 'dart:collection';

import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/journey_info.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:easy_localization/easy_localization.dart';

class JourneyUiBody extends StatefulWidget {
  const JourneyUiBody({super.key});

  @override
  State<JourneyUiBody> createState() => _JourneyUiBodyState();
}

class _JourneyUiBodyState extends State<JourneyUiBody> {
  final ValueNotifier<List<JourneyHeader>> _journeyHeaderList =
      ValueNotifier<List<JourneyHeader>>([]);

  List<DateTime?> _singleSelectedDatePickerValue = [
    DateTime.now(),
  ];
  late final DateTime? firstDate;
  final lastDate = DateTime.now();
  late Map<int, dynamic> nestedYearMonthsAndDay = {};
  bool _isLoadingFirstDate = true;

  @override
  void initState() {
    super.initState();
    _initialize();
    _updateJourneyHeaderList();
  }

  Future<void> _initialize() async {
    NaiveDate? earliestDate = await api.earliestJourneyDate();
    if (earliestDate != null) {
      firstDate = DateTime.parse(naiveDateToString(date: earliestDate));
    } else {
      firstDate = null;
    }

    var yearList = await api.yearsWithJourney();
    for (var year in yearList) {
      var monthsList = await api.monthsWithJourney(year: year);
      var tmp = {};
      for (var month in monthsList) {
        var dayList = await api.daysWithJourney(year: year, month: month);
        tmp[month] = dayList;
      }
      nestedYearMonthsAndDay[year] = tmp;
    }
    setState(() {
      _isLoadingFirstDate = false;
    });
  }

  void _updateJourneyHeaderList() async {
    _journeyHeaderList.value = await api.listJournyOnDate(
        year: _singleSelectedDatePickerValue.first!.year,
        month: _singleSelectedDatePickerValue.first!.month,
        day: _singleSelectedDatePickerValue.first!.day);
  }

  Widget _buildDatePickerWithValue() {
    final config = CalendarDatePicker2Config(
      firstDate: firstDate,
      lastDate: DateTime.now(),
      centerAlignModePicker: true,
      calendarType: CalendarDatePicker2Type.single,
      selectedDayHighlightColor: Colors.teal[800],
      weekdayLabelTextStyle: const TextStyle(
        color: Colors.black87,
        fontWeight: FontWeight.bold,
      ),
      controlsTextStyle: const TextStyle(
        color: Colors.black,
        fontSize: 15,
        fontWeight: FontWeight.bold,
      ),
      selectableYearPredicate: (year) =>
          nestedYearMonthsAndDay.containsKey(year),
      selectableMonthPredicate: (year, month) {
        if (!nestedYearMonthsAndDay.containsKey(year)) return false;
        return nestedYearMonthsAndDay[year]!.containsKey(month);
      },
      selectableDayPredicate: (day) {
        if (nestedYearMonthsAndDay.containsKey(day.year) &&
            nestedYearMonthsAndDay[day.year]!.containsKey(day.month)) {
          return nestedYearMonthsAndDay[day.year][day.month].contains(day.day);
        }
        return false;
      },
      dayBuilder: ({
        required date,
        textStyle,
        decoration,
        isSelected,
        isDisabled,
        isToday,
      }) {
        Widget? dayWidget;
        if (nestedYearMonthsAndDay.containsKey(date.year) &&
            nestedYearMonthsAndDay[date.year]!.containsKey(date.month) &&
            nestedYearMonthsAndDay[date.year][date.month].contains(date.day)) {
          dayWidget = Container(
            decoration: decoration,
            child: Center(
              child: Stack(
                alignment: AlignmentDirectional.center,
                children: [
                  Text(
                    MaterialLocalizations.of(context).formatDecimal(date.day),
                    style: textStyle,
                  ),
                  Padding(
                    padding: const EdgeInsets.only(top: 27.5),
                    child: Container(
                      height: 4,
                      width: 4,
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(5),
                        color: Colors.cyan[200],
                      ),
                    ),
                  ),
                ],
              ),
            ),
          );
        }
        return dayWidget;
      },
      // 渲染存在数据的年 月 日
      // yearBuilder: ({
      //   required year,
      //   decoration,
      //   isCurrentYear,
      //   isDisabled,
      //   isSelected,
      //   textStyle,
      // }) {
      //   return Container();
      // },
      dynamicCalendarRows: true,
      disabledDayTextStyle:
          const TextStyle(color: Colors.grey, fontWeight: FontWeight.w400),
      disabledMonthTextStyle:
          const TextStyle(color: Colors.grey, fontWeight: FontWeight.w400),
      disabledYearTextStyle:
          const TextStyle(color: Colors.grey, fontWeight: FontWeight.w400),
    );
    return CalendarDatePicker2(
      config: config,
      value: _singleSelectedDatePickerValue,
      onValueChanged: (dates) {
        setState(() => _singleSelectedDatePickerValue = dates);
        _updateJourneyHeaderList();
      },
      onDisplayedMonthChanged: (value) async {
        DateTime nextDate = DateTime(
            value.year, value.month, _singleSelectedDatePickerValue.first!.day);
        if (lastDate.isBefore(nextDate)) {
          nextDate = lastDate;
        }
        if (firstDate!.isAfter(nextDate)) {
          nextDate = firstDate!;
        }
        setState(() {
          _singleSelectedDatePickerValue = [nextDate];
        });
        _updateJourneyHeaderList();
      },
    );
  }

  Widget _buildJourneyHeaderList() {
    return Expanded(
      child: ValueListenableBuilder<List<JourneyHeader>>(
        valueListenable: _journeyHeaderList,
        builder: (context, value, _) {
          return ListView.builder(
            itemCount: value.length,
            itemBuilder: (context, index) {
              return Container(
                margin: const EdgeInsets.symmetric(
                  horizontal: 12.0,
                  vertical: 4.0,
                ),
                decoration: BoxDecoration(
                  border: Border.all(),
                  borderRadius: BorderRadius.circular(12.0),
                ),
                child: ListTile(
                  title:
                      Text(naiveDateToString(date: value[index].journeyDate)),
                  onTap: () {
                    Navigator.push(context, MaterialPageRoute(
                      builder: (context) {
                        return JourneyInfoPage(
                          journeyHeader: value[index],
                        );
                      },
                    )).then((refresh) async {
                      if (refresh != null && refresh) {
                        _updateJourneyHeaderList();
                      }
                    });
                  },
                ),
              );
            },
          );
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_isLoadingFirstDate) {
      // 渲染加载状态
      return const Center(child: CircularProgressIndicator());
    }
    if (firstDate == null) {
      return Scaffold(
          body: Center(
        child: Text(context.tr("journey.empty_journey_data")),
      ));
    } else {
      return Scaffold(
          body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          _buildDatePickerWithValue(),
          const SizedBox(height: 8.0),
          _buildJourneyHeaderList(),
        ],
      ));
    }
  }
}
