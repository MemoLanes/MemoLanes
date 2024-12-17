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
  List<JourneyHeader> _journeyHeaderList = [];

  DateTime _selectedDate = DateTime.now();
  late final DateTime? _firstDate;
  final lastDate = DateTime.now();
  late final List<int> _yearsWithJourneyList;
  late List<int> _monthsWithJourneyList;
  late List<int> _daysWithJourneyList;
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
      _firstDate = DateTime.parse(naiveDateToString(date: earliestDate));
    } else {
      _firstDate = null;
    }
    _yearsWithJourneyList = await api.yearsWithJourney();
    _monthsWithJourneyList =
        await api.monthsWithJourney(year: _selectedDate.year);
    _daysWithJourneyList = await api.daysWithJourney(
        year: _selectedDate.year, month: _selectedDate.month);
    setState(() {
      _isLoadingFirstDate = false;
    });
  }

  void _updateJourneyHeaderList() async {
    final journeyHeaderList = await api.listJournyOnDate(
        year: _selectedDate.year,
        month: _selectedDate.month,
        day: _selectedDate.day);
    setState(() {
      _journeyHeaderList = journeyHeaderList;
    });
  }

  Widget _buildDatePickerWithValue(DateTime firstDate) {
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
      selectableYearPredicate: (year) => _yearsWithJourneyList.contains(year),
      selectableMonthPredicate: (year, month) =>
          _monthsWithJourneyList.contains(month),
      selectableDayPredicate: (day) => _daysWithJourneyList.contains(day.day),
      dayBuilder: ({
        required date,
        textStyle,
        decoration,
        isSelected,
        isDisabled,
        isToday,
      }) {
        Widget? dayWidget;
        if (_daysWithJourneyList.contains(date.day)) {
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
                        color: const Color(0xFFB4EC51),
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
      value: [_selectedDate],
      onValueChanged: (dates) {
        setState(() => _selectedDate = dates.first);
        _updateJourneyHeaderList();
      },
      onDisplayedMonthChanged: (value) async {
        DateTime nextDate =
            DateTime(value.year, value.month, _selectedDate.day);
        if (lastDate.isBefore(nextDate)) {
          nextDate = lastDate;
        }
        if (firstDate.isAfter(nextDate)) {
          nextDate = firstDate;
        }
        if (value.year != _selectedDate.year) {
          _monthsWithJourneyList =
              await api.monthsWithJourney(year: nextDate.year);
        }

        _daysWithJourneyList = await api.daysWithJourney(
            month: nextDate.month, year: nextDate.year);
        setState(() {
          _selectedDate = nextDate;
        });
        _updateJourneyHeaderList();
      },
    );
  }

  Widget _buildJourneyHeaderList() {
    return Expanded(
      child: ListView.builder(
          itemCount: _journeyHeaderList.length,
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
                title: Text(naiveDateToString(
                    date: _journeyHeaderList[index].journeyDate)),
                onTap: () {
                  Navigator.push(context, MaterialPageRoute(
                    builder: (context) {
                      return JourneyInfoPage(
                        journeyHeader: _journeyHeaderList[index],
                      );
                    },
                  )).then((refresh) async {
                    if (refresh != null && refresh) {
                      _daysWithJourneyList = await api.daysWithJourney(
                          year: _selectedDate.year, month: _selectedDate.month);
                      _updateJourneyHeaderList();
                    }
                  });
                },
              ),
            );
          }),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_isLoadingFirstDate) {
      return const Center(child: CircularProgressIndicator());
    }
    final firstDate = _firstDate;
    if (firstDate == null) {
      return Scaffold(
          body: Center(
        child: Text(context.tr("journey.no_data")),
      ));
    } else {
      return Scaffold(
          body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          _buildDatePickerWithValue(firstDate),
          const SizedBox(height: 8.0),
          _buildJourneyHeaderList(),
        ],
      ));
    }
  }
}
